//! IMU dead reckoning between lidar fixes.
//!
//! The accelerometer path is intentionally short-term only: double integration
//! accumulates error quickly. The gyro heading is the useful part of this
//! estimate. Wheel odometry, when it becomes available, should replace the
//! acceleration integration at the hook provided by [`DeadReckoning::update`].

use super::{
    GYRO_Z_CCW_POSITIVE, LocalPose, LocalPosition, heading_to_degrees, to_global, wrap_2pi,
};
use crate::messages::router;

const GRAVITY: units::Acceleration = units::Acceleration::new(9.81);
const GRAVITY_TOLERANCE: units::Acceleration = units::Acceleration::new(0.15);
const STATIONARY_GYRO_LIMIT: units::AngularVelocity = units::AngularVelocity::new(0.02);
const STATIONARY_SAMPLES_REQUIRED: u16 = 50;
const BIAS_ALPHA: units::Scalar = units::Scalar::new(0.01);
const VELOCITY_DAMPING: units::Scalar = units::Scalar::new(0.98);
const ZERO_TIME: units::Time = units::Time::new(0.0);
const ZERO_ANGLE: units::PlaneAngle = units::PlaneAngle::new(0.0);

/// The local pose and short-term inertial state maintained between fixes.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(super) struct DeadReckoning {
    pub(super) pose: LocalPose,
    pub(super) velocity: (units::Velocity, units::Velocity),
    pub(super) last_imu_timestamp: Option<units::Time>,
    pub(super) gyro_bias_z: units::AngularVelocity,
    pub(super) accel_bias: (units::Acceleration, units::Acceleration),
    pub(super) stationary_samples: u16,
    last_output_timestamp: Option<units::Time>,
}

impl DeadReckoning {
    /// Seed the local estimate from a host-provided position.
    pub(super) fn from_position(
        position: &router::Position,
        frame: &router::ReferenceFrame,
    ) -> Self {
        Self::from_local_pose(LocalPose {
            position: super::to_local(frame, &position.coordinate),
            heading: wrap_2pi(
                units::PlaneAngle::new(1.0)
                    * (position.heading / units::imperial::Degrees::new(1.0)),
            ),
        })
    }

    /// Seed an inertial state from a lidar-only local fix.
    pub(super) fn from_local_pose(pose: LocalPose) -> Self {
        Self {
            pose,
            velocity: (units::Velocity::new(0.0), units::Velocity::new(0.0)),
            last_imu_timestamp: None,
            gyro_bias_z: units::AngularVelocity::new(0.0),
            accel_bias: (units::Acceleration::new(0.0), units::Acceleration::new(0.0)),
            stationary_samples: 0,
            last_output_timestamp: None,
        }
    }

    pub(super) fn pose(&self) -> LocalPose {
        self.pose
    }

    /// Replace the authoritative pose after a lidar correction.
    pub(super) fn set_pose(&mut self, pose: LocalPose) {
        self.pose = pose;
        self.velocity = (units::Velocity::new(0.0), units::Velocity::new(0.0));
    }

    pub(super) fn position(
        &self,
        timestamp: units::Time,
        frame: &router::ReferenceFrame,
    ) -> router::Position {
        router::Position {
            timestamp,
            coordinate: to_global(frame, self.pose.position),
            heading: heading_to_degrees(self.pose.heading),
        }
    }

    /// Advance the estimate with one IMU sample and emit throttled navigation output.
    pub(super) fn update(
        &mut self,
        timestamp: units::Time,
        imu: router::Imu,
        frame: &router::ReferenceFrame,
    ) -> Option<router::Position> {
        let previous_timestamp = self.last_imu_timestamp.replace(timestamp)?;
        let elapsed = timestamp - previous_timestamp;
        if !super::is_finite_time(elapsed) || elapsed <= ZERO_TIME {
            return None;
        }
        let dt = if elapsed < super::MAX_DT {
            elapsed
        } else {
            super::MAX_DT
        };

        let stationary = is_stationary(&imu);
        if stationary {
            self.stationary_samples = self.stationary_samples.saturating_add(1);
            if self.stationary_samples >= STATIONARY_SAMPLES_REQUIRED {
                self.gyro_bias_z = low_pass(self.gyro_bias_z, imu.gyro.z);
                self.accel_bias.0 = low_pass(self.accel_bias.0, imu.accel.x);
                self.accel_bias.1 = low_pass(self.accel_bias.1, imu.accel.y);
            }
        } else {
            self.stationary_samples = 0;
        }

        let yaw_rate = imu.gyro.z - self.gyro_bias_z;
        let heading_delta = if GYRO_Z_CCW_POSITIVE {
            ZERO_ANGLE - yaw_rate * dt
        } else {
            yaw_rate * dt
        };
        self.pose.heading = wrap_2pi(self.pose.heading + heading_delta);

        if stationary {
            // Zero-velocity update: a stationary platform is a strong correction
            // against the acceleration integrator's otherwise unbounded drift.
            self.velocity = (units::Velocity::new(0.0), units::Velocity::new(0.0));
        } else {
            // Hook for wheel odometry: replace this body-acceleration integration
            // with measured drive-base velocity when that source is available.
            let corrected_x = imu.accel.x - self.accel_bias.0;
            let corrected_y = imu.accel.y - self.accel_bias.1;
            let east_acceleration =
                corrected_x * self.pose.heading.sin() - corrected_y * self.pose.heading.cos();
            let north_acceleration =
                corrected_x * self.pose.heading.cos() + corrected_y * self.pose.heading.sin();
            self.velocity = (
                (self.velocity.0 + east_acceleration * dt) * VELOCITY_DAMPING,
                (self.velocity.1 + north_acceleration * dt) * VELOCITY_DAMPING,
            );

            self.pose.position = LocalPosition {
                east: self.pose.position.east + self.velocity.0 * dt,
                north: self.pose.position.north + self.velocity.1 * dt,
            };
        }

        if should_emit(self.last_output_timestamp, timestamp) {
            self.last_output_timestamp = Some(timestamp);
            Some(router::Position {
                timestamp,
                coordinate: to_global(frame, self.pose.position),
                heading: heading_to_degrees(self.pose.heading),
            })
        } else {
            None
        }
    }
}

/// Stationary when the specific-force magnitude matches gravity and the body
/// rotation rate is small. Using vector norms (rather than per-axis limits)
/// keeps a slightly tilted or biased accelerometer eligible for ZUPT and bias
/// estimation.
fn is_stationary(imu: &router::Imu) -> bool {
    const ONE_ACCEL: units::Acceleration = units::Acceleration::new(1.0);
    const ONE_RATE: units::AngularVelocity = units::AngularVelocity::new(1.0);

    let accel_norm = ONE_ACCEL
        * norm(
            imu.accel.x / ONE_ACCEL,
            imu.accel.y / ONE_ACCEL,
            imu.accel.z / ONE_ACCEL,
        );
    let gyro_norm = ONE_RATE
        * norm(
            imu.gyro.x / ONE_RATE,
            imu.gyro.y / ONE_RATE,
            imu.gyro.z / ONE_RATE,
        );
    (accel_norm - GRAVITY).abs() <= GRAVITY_TOLERANCE && gyro_norm < STATIONARY_GYRO_LIMIT
}

fn norm(x: units::Scalar, y: units::Scalar, z: units::Scalar) -> units::Scalar {
    (x * x + y * y + z * z).sqrt()
}

fn should_emit(last_output: Option<units::Time>, timestamp: units::Time) -> bool {
    let Some(last_output) = last_output else {
        return true;
    };
    super::is_finite_time(timestamp - last_output)
        && timestamp - last_output >= super::DR_OUTPUT_PERIOD
}

fn low_pass<T>(previous: T, sample: T) -> T
where
    T: Copy
        + core::ops::Sub<T, Output = T>
        + core::ops::Mul<units::Scalar, Output = T>
        + core::ops::Add<T, Output = T>,
{
    previous + (sample - previous) * BIAS_ALPHA
}
