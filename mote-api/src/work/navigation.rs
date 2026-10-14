// This needs some sample lidar scans
// Work: from the lidar scans determine position (including heading)

use crate::messages::router;

mod corner;
mod dead_reckoning;
mod fix;
mod frame;
mod revolution;

pub use corner::{CornerKind, CornerObservation, detect_corners};
pub use fix::fix_from_corners;
pub use frame::{
    LocalPose, LocalPosition, METRES_PER_DEGREE, baseline_bearing, diagonal_scale,
    heading_to_degrees, lidar_to_robot, northeast_local, to_global, to_local, wrap_2pi, wrap_pi,
};
use revolution::ONE_DEGREE;
pub use revolution::{
    BIN_COUNT, EXTENDED_BIN_COUNT, MIN_BINS, OVERLAP_BIN_COUNT, REVOLUTION_POINT_CAPACITY,
    Revolution, RevolutionPoints,
};

/// Mean earth radius, retained for the eventual frame-conversion implementation.
pub const EARTH_RADIUS_M: units::Length = units::Length::new(6_371_000.0);

/// Interior angle of the southwest reference structure.
pub const SW_INTERIOR_ANGLE: units::PlaneAngle =
    units::PlaneAngle::new(2.0 * core::f32::consts::PI / 3.0);

/// Interior angle of the northeast reference structure.
///
/// Both structures are 120-degree isosceles triangles; the robot sees the
/// northeast one from its concave side.
pub const NE_INTERIOR_ANGLE: units::PlaneAngle =
    units::PlaneAngle::new(2.0 * core::f32::consts::PI / 3.0);

/// Largest angle between a structure's bisector and the robot's line of sight.
///
/// The arena is square and each structure's bisector lies along its diagonal,
/// so the line of sight is on the bisector from the centre and 45 degrees
/// off it from the southeast and northwest corners.
pub const MAX_OFF_AXIS_VIEW: units::PlaneAngle =
    units::PlaneAngle::new(core::f32::consts::FRAC_PI_4);

/// Points a structure's run may have beyond its expected edge-to-edge count.
///
/// Covers bins that only partly overlap an edge and the lidar's beam width.
pub const WIDTH_SLACK_POINTS: units::Scalar = units::Scalar::new(2.0);

/// C1: lidar angle zero is aligned with the robot's forward axis.
pub const LIDAR_ZERO_IS_FORWARD: bool = true;

/// C1: lidar angles increase clockwise when viewed from above.
pub const LIDAR_CLOCKWISE: bool = true;

/// C2: position headings use the compass convention: north is zero and clockwise is positive.
pub const HEADING_IS_COMPASS: bool = true;

/// C2: local north is aligned with true north in the flat-earth frame.
pub const LOCAL_NORTH_IS_TRUE_NORTH: bool = true;

/// C5: the gyro z-axis reports positive angular velocity for counter-clockwise yaw.
pub const GYRO_Z_CCW_POSITIVE: bool = true;

/// Maximum interval represented by one dead-reckoning integration step.
pub const MAX_DT: units::Time = units::Time::new(0.1);

/// Minimum interval between emitted dead-reckoning positions.
pub const DR_OUTPUT_PERIOD: units::Time = units::Time::new(0.1);

/// Minimum accepted lidar return range.
pub const MIN_RANGE: units::Length = units::Length::new(0.05);

/// Maximum accepted lidar return range.
pub const MAX_RANGE: units::Length = units::Length::new(6.0);

/// Range jump between adjacent bins that marks the edge of a reference structure.
///
/// The structures stand at least 0.25 m in front of anything behind them; the
/// threshold sits below that to tolerate range noise, and well above the
/// bin-to-bin range change along a wall.
pub const EDGE_JUMP: units::Length = units::Length::new(0.2);

/// Smallest accepted ratio of the shorter to the longer leg of a structure.
pub const MIN_LEG_RATIO: units::Scalar = units::Scalar::new(0.7);

/// Largest acceptable total-least-squares wall-fit residual.
pub const MAX_RMS: units::Length = units::Length::new(0.02);

/// Maximum deviation from a structure's nominal interior angle.
pub const ANGLE_TOL: units::PlaneAngle =
    units::PlaneAngle::new(15.0 * core::f32::consts::PI / 180.0);

/// Maximum disagreement between position fixes from the two corners.
///
/// Both fixes share the baseline heading, so this bounds the difference between
/// the measured and expected vertex separation.
pub const MAX_FIX_DISAGREEMENT: units::Length = units::Length::new(0.05);

/// How long a corner observation is retained for pairing with the other corner.
///
/// A single revolution does not always see both structures, so a fix may
/// combine the latest southwest and northeast observations from different
/// revolutions, provided neither is older than this.
pub const CORNER_MEMORY: units::Time = units::Time::new(1.0);

/// Maximum allowed position jump when fusing a fix with dead reckoning.
pub const MAX_JUMP: units::Length = units::Length::new(0.5);

const MAX_FINITE: units::Scalar = units::Scalar::new(core::f32::MAX);
const MIN_FINITE: units::Scalar = units::Scalar::new(-core::f32::MAX);

pub(crate) fn is_finite_scalar(value: units::Scalar) -> bool {
    value >= MIN_FINITE && value <= MAX_FINITE
}

pub(crate) fn is_finite_length(value: units::Length) -> bool {
    is_finite_scalar(value / units::Length::new(1.0))
}

pub(crate) fn is_finite_angle(value: units::PlaneAngle) -> bool {
    is_finite_scalar(value / units::PlaneAngle::new(1.0))
}

pub(crate) fn is_finite_time(value: units::Time) -> bool {
    is_finite_scalar(value / units::Time::new(1.0))
}

/// The result of processing one accepted lidar revolution.
pub struct NavigationReport {
    /// Timestamp of the first accepted point in the revolution.
    pub timestamp: units::Time,
    /// Number of populated bins in the revolution.
    pub filled_bins: u16,
    /// Geometrically verified corner observations.
    pub corners: heapless::Vec<CornerObservation, 4>,
    /// The fix accepted from this revolution, if one was available.
    pub pose: Option<router::Position>,
}

/// Context for doing the navigation work.
#[derive(Default)]
pub struct Context {
    current_revolution: Option<Revolution>,
    completed: heapless::Deque<Revolution, 4>,
    reports: heapless::Deque<NavigationReport, 4>,
    revolution_wraps: u32,
    accepted_revolutions: u32,
    discarded_revolutions: u32,
    accepted_points: u32,
    discarded_points: u32,
    dead_reckoning: Option<dead_reckoning::DeadReckoning>,
    last_accepted_fix_timestamp: Option<units::Time>,
    recent_southwest: Option<(units::Time, CornerObservation)>,
    recent_northeast: Option<(units::Time, CornerObservation)>,
}

impl Context {
    /// Advance navigation with an optional LiDAR scan and IMU sample.
    pub fn update(
        &mut self,
        timestamp: units::Time,
        position: Option<&router::Position>,
        scan: Option<router::Scan>,
        imu: Option<router::Imu>,
        reference_frame: &router::ReferenceFrame,
        status: &mut router::Status,
    ) -> Option<router::Position> {
        if self.dead_reckoning.is_none() {
            if let Some(position) = position {
                self.dead_reckoning = Some(dead_reckoning::DeadReckoning::from_position(
                    position,
                    reference_frame,
                ));
            }
        }

        let dead_reckoning_position = imu.and_then(|imu| {
            self.dead_reckoning
                .as_mut()
                .and_then(|dead_reckoning| dead_reckoning.update(timestamp, imu, reference_frame))
        });

        let Some(scan) = scan else {
            router::update_counter(
                &mut status
                    .counters
                    .navigation_dead_reckoning_calcs_since_lidar_fix,
                1,
            );
            return dead_reckoning_position;
        };

        let accepted_revolutions_before = self.accepted_revolutions;
        for point in scan.point_cloud {
            if !is_finite_angle(point.angle)
                || !is_finite_length(point.distance)
                || point.distance < MIN_RANGE
                || point.distance > MAX_RANGE
            {
                self.discarded_points += 1;
                continue;
            }

            self.accepted_points += 1;
            let angle = wrap_2pi(point.angle);
            let wraps = self
                .current_revolution
                .as_ref()
                .map(|revolution| {
                    revolution.last_angle() - angle > units::PlaneAngle::new(core::f32::consts::PI)
                })
                .unwrap_or(false);
            if wraps {
                router::update_counter(&mut status.counters.navigation_lidar_revolutions, 1);
                self.finish_revolution(status, timestamp, reference_frame);
            }

            self.current_revolution
                .get_or_insert_with(|| Revolution::new(timestamp))
                .insert(angle, point.distance);
        }

        let fix_position = (self.accepted_revolutions > accepted_revolutions_before)
            .then(|| self.reports.back().and_then(|report| report.pose.clone()))
            .flatten();
        if fix_position.is_some() {
            router::update_counter(&mut status.counters.navigation_lidar_scan_fix, 1);
            status
                .counters
                .navigation_dead_reckoning_calcs_since_lidar_fix = 0;
        } else {
            router::update_counter(
                &mut status
                    .counters
                    .navigation_dead_reckoning_calcs_since_lidar_fix,
                1,
            );
        }
        fix_position.or(dead_reckoning_position)
    }

    fn finish_revolution(
        &mut self,
        status: &mut router::Status,
        timestamp: units::Time,
        reference_frame: &router::ReferenceFrame,
    ) {
        let Some(revolution) = self.current_revolution.take() else {
            return;
        };
        self.revolution_wraps += 1;
        if revolution.filled_bins() < MIN_BINS {
            self.discarded_revolutions += 1;
            return;
        }

        self.accepted_revolutions += 1;
        let report = self.process_revolution(status, &revolution, timestamp, reference_frame);
        if let Err(revolution) = self.completed.push_back(revolution) {
            let _ = self.completed.pop_front();
            let _ = self.completed.push_back(revolution);
        }
        if let Err(report) = self.reports.push_back(report) {
            let _ = self.reports.pop_front();
            let _ = self.reports.push_back(report);
        }
    }

    fn process_revolution(
        &mut self,
        status: &mut router::Status,
        revolution: &Revolution,
        timestamp: units::Time,
        reference_frame: &router::ReferenceFrame,
    ) -> NavigationReport {
        let corners = detect_corners(revolution, reference_frame.structure_side_wall_length);
        for corner in corners.iter() {
            match &corner.kind {
                CornerKind::Northeast => {
                    router::update_counter(&mut status.counters.navigation_ne_corners, 1);
                    self.recent_northeast = Some((timestamp, *corner));
                }
                CornerKind::Southwest => {
                    router::update_counter(&mut status.counters.navigation_sw_corners, 1);
                    self.recent_southwest = Some((timestamp, *corner));
                }
            }
        }
        let recent_corners = self.recent_corners(timestamp);
        if recent_corners.len() == 2 {
            router::update_counter(
                &mut status.counters.navigation_lidar_both_corners_with_recent,
                1,
            );
        }
        let prior_heading = self
            .dead_reckoning
            .as_ref()
            .map(|dead_reckoning| dead_reckoning.pose().heading);
        let pose = fix_from_corners(reference_frame, &recent_corners, prior_heading)
            .and_then(|fix| self.accept_fix(fix, timestamp, reference_frame));
        if pose.is_some() {
            router::update_counter(&mut status.counters.navigation_lidar_fix_from_corners, 1);
        }
        NavigationReport {
            timestamp: revolution.start_timestamp(),
            filled_bins: revolution.filled_bins(),
            corners,
            pose,
        }
    }

    /// Latest observation of each corner seen within [`CORNER_MEMORY`] of `timestamp`.
    ///
    /// Expired observations are forgotten.
    fn recent_corners(&mut self, timestamp: units::Time) -> heapless::Vec<CornerObservation, 2> {
        let mut corners = heapless::Vec::new();
        for recent in [&mut self.recent_southwest, &mut self.recent_northeast] {
            match recent {
                Some((seen, corner)) if timestamp - *seen <= CORNER_MEMORY => {
                    let _ = corners.push(*corner);
                }
                _ => *recent = None,
            }
        }
        corners
    }

    fn accept_fix(
        &mut self,
        fix: LocalPose,
        timestamp: units::Time,
        reference_frame: &router::ReferenceFrame,
    ) -> Option<router::Position> {
        let pose = if let Some(dead_reckoning) = self.dead_reckoning.as_ref() {
            let prior = dead_reckoning.pose();
            let has_recent_fix = self
                .last_accepted_fix_timestamp
                .map(|last| timestamp - last <= units::Time::new(5.0))
                .unwrap_or(false);
            if has_recent_fix
                && (local_distance(prior.position, fix.position) > MAX_JUMP
                    || heading_difference(prior.heading, fix.heading)
                        > units::PlaneAngle::new(core::f32::consts::FRAC_PI_4))
            {
                return None;
            }
            if self.last_accepted_fix_timestamp.is_some() {
                blend_pose(prior, fix, units::Scalar::new(0.7), units::Scalar::new(0.5))
            } else {
                fix
            }
        } else {
            fix
        };

        let dead_reckoning = self
            .dead_reckoning
            .get_or_insert_with(|| dead_reckoning::DeadReckoning::from_local_pose(pose));
        dead_reckoning.set_pose(pose);
        self.last_accepted_fix_timestamp = Some(timestamp);
        Some(dead_reckoning.position(timestamp, reference_frame))
    }

    /// Remove the oldest accepted revolution, if one is waiting for processing.
    pub fn take_completed_revolution(&mut self) -> Option<Revolution> {
        self.completed.pop_front()
    }

    /// Remove the oldest integrated navigation report, if one is waiting.
    pub fn take_navigation_report(&mut self) -> Option<NavigationReport> {
        self.reports.pop_front()
    }

    /// View the revolution currently being assembled, if any.
    pub fn current_revolution(&self) -> Option<&Revolution> {
        self.current_revolution.as_ref()
    }

    /// Number of angle-wrap boundaries observed in the input stream.
    pub const fn revolution_wraps(&self) -> u32 {
        self.revolution_wraps
    }

    /// Number of wrapped revolutions that met [`MIN_BINS`].
    pub const fn accepted_revolutions(&self) -> u32 {
        self.accepted_revolutions
    }

    /// Number of wrapped revolutions discarded for insufficient angular coverage.
    pub const fn discarded_revolutions(&self) -> u32 {
        self.discarded_revolutions
    }

    /// Number of lidar points accepted after quality and range filtering.
    pub const fn accepted_points(&self) -> u32 {
        self.accepted_points
    }

    /// Number of lidar points rejected by quality or range filtering.
    pub const fn discarded_points(&self) -> u32 {
        self.discarded_points
    }
}

fn local_distance(first: LocalPosition, second: LocalPosition) -> units::Length {
    ((first.east - second.east) * (first.east - second.east)
        + (first.north - second.north) * (first.north - second.north))
        .sqrt()
}

fn heading_difference(first: units::PlaneAngle, second: units::PlaneAngle) -> units::PlaneAngle {
    wrap_pi(first - second).abs()
}

fn blend_pose(
    dead_reckoned: LocalPose,
    fix: LocalPose,
    position_alpha: units::Scalar,
    heading_alpha: units::Scalar,
) -> LocalPose {
    let east = dead_reckoned.position.east
        + (fix.position.east - dead_reckoned.position.east) * position_alpha;
    let north = dead_reckoned.position.north
        + (fix.position.north - dead_reckoned.position.north) * position_alpha;
    let heading_error = wrap_pi(fix.heading - dead_reckoned.heading);
    LocalPose {
        position: LocalPosition { east, north },
        heading: wrap_2pi(dead_reckoned.heading + heading_error * heading_alpha),
    }
}
