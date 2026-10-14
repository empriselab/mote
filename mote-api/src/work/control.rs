//! Differential-drive control.
//!
//! Turns a [`router::Guidance`] into left and right wheel speed commands
//! using a simple proportional steering law:
//!
//! - The turn component is proportional to the relative heading error and
//!   saturates at [`MAX_TURN_SPEED`]. It is added to the left wheel and
//!   subtracted from the right wheel.
//! - The forward component is the approach speed scaled by the cosine of the
//!   heading error, so the robot slows while turning and pivots in place when
//!   the target is abeam or behind it.
//! - The approach speed covers the remaining distance in [`APPROACH_TIME`],
//!   limited to between [`MIN_APPROACH_SPEED`] and [`CRUISE_SPEED`], so the
//!   robot slows as it closes on a waypoint.
//! - If either wheel would exceed [`MAX_WHEEL_SPEED`], both wheels are scaled
//!   down together so the turn radius is preserved.
//!
//! Guidance distances are in global metres, but the wheels move the robot in
//! local (lidar) metres, so the distance is converted with the reference
//! frame's diagonal scale before it is turned into a wheel speed. Headings
//! need no conversion: the scale is uniform and local north is true north.
//!
//! Guidance headings use the compass convention (clockwise positive), so a
//! positive heading means the target is to the right and the robot turns right
//! by driving the left wheel faster. Positive wheel speeds drive forward; the
//! firmware handles any motor inversion.

use crate::messages::router;
use crate::work::navigation::diagonal_scale;

/// Forward wheel speed when the target is straight ahead and far away.
pub const CRUISE_SPEED: units::AngularVelocity = units::AngularVelocity::new(1.5);
/// Slowest forward wheel speed while approaching a target, so the robot
/// keeps moving instead of stalling just short of a waypoint.
pub const MIN_APPROACH_SPEED: units::AngularVelocity = units::AngularVelocity::new(0.5);
/// Time in which the approach speed would cover the remaining local distance.
/// Larger values start slowing down further from the target.
pub const APPROACH_TIME: units::Time = units::Time::new(0.5);
/// Drive wheel diameter. Half of it is the radius that converts local metres
/// travelled into wheel radians.
pub const WHEEL_DIAMETER: units::imperial::Inch = units::imperial::Inch::new(2.5);
/// Largest turn component applied to the wheels.
pub const MAX_TURN_SPEED: units::AngularVelocity = units::AngularVelocity::new(1.0);
/// Largest speed commanded to either wheel.
pub const MAX_WHEEL_SPEED: units::AngularVelocity = units::AngularVelocity::new(1.5);
/// Turn component, in rad/s of wheel speed, per radian of heading error.
pub const STEERING_GAIN: f32 = 2.0;

#[derive(Default)]
/// Context for doing the control work
pub struct Context {}

impl Context {
    /// Function for updating the data
    ///
    /// Always produces a command. Without usable guidance (no guidance, a
    /// non-finite heading or distance, or a reference frame without positive
    /// diagonals) the wheels are commanded to stop, so the robot halts at the
    /// end of the route instead of continuing on its last command until the
    /// drive-base watchdog expires.
    pub fn update(
        &mut self,
        timestamp: units::Time,
        guidance_message: Option<router::Guidance>,
        reference_frame: &router::ReferenceFrame,
        // The status log is left untouched so guidance messages written in the
        // same update, such as route completion, are not overwritten.
        _debug: &mut router::Status,
    ) -> Option<router::Control> {
        let motor_drive = guidance_message
            .as_ref()
            .and_then(|guidance| steer(guidance, reference_frame))
            .unwrap_or_else(stop);
        Some(router::Control {
            timestamp,
            motor_drive,
        })
    }
}

fn steer(
    guidance: &router::Guidance,
    reference_frame: &router::ReferenceFrame,
) -> Option<router::MotorDrive> {
    let heading: f32 = guidance.heading.into();
    let global_distance: f32 = guidance.distance.into();
    let scale: f32 = diagonal_scale(reference_frame).into();
    let local_distance = global_distance * scale;
    if !heading.is_finite() || !local_distance.is_finite() || scale <= 0.0 {
        return None;
    }
    // Wrap to [-pi, pi] so the robot always takes the shorter turn.
    let heading = libm::atan2f(libm::sinf(heading), libm::cosf(heading));

    let cruise: f32 = CRUISE_SPEED.into();
    let min_approach: f32 = MIN_APPROACH_SPEED.into();
    let approach_time: f32 = APPROACH_TIME.into();
    let wheel_radius = wheel_radius_metres();
    let max_turn: f32 = MAX_TURN_SPEED.into();
    let max_wheel: f32 = MAX_WHEEL_SPEED.into();

    let approach = (local_distance / (wheel_radius * approach_time)).clamp(min_approach, cruise);
    let forward = approach * libm::cosf(heading).max(0.0);
    let turn = (STEERING_GAIN * heading).clamp(-max_turn, max_turn);
    let (left, right) = (forward + turn, forward - turn);

    let largest = left.abs().max(right.abs());
    let scale = if largest > max_wheel {
        max_wheel / largest
    } else {
        1.0
    };

    Some(router::MotorDrive {
        left: units::AngularVelocity::new(left * scale),
        right: units::AngularVelocity::new(right * scale),
    })
}

fn wheel_radius_metres() -> f32 {
    let diameter: units::Length = WHEEL_DIAMETER.into();
    let diameter: f32 = diameter.into();
    0.5 * diameter
}

fn stop() -> router::MotorDrive {
    router::MotorDrive {
        left: units::AngularVelocity::new(0.0),
        right: units::AngularVelocity::new(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One local metre per global metre.
    fn unit_frame() -> router::ReferenceFrame {
        frame(1.0, 1.0)
    }

    fn frame(global_diagonal: f32, local_diagonal: f32) -> router::ReferenceFrame {
        router::ReferenceFrame {
            global_diagonal: units::Length::new(global_diagonal),
            local_diagonal: units::Length::new(local_diagonal),
            ..router::ReferenceFrame::default()
        }
    }

    /// Guidance toward a target far enough away to drive at cruise speed.
    fn guidance(heading_degrees: f32) -> router::Guidance {
        guidance_at(heading_degrees, 100.0)
    }

    fn guidance_at(heading_degrees: f32, distance: f32) -> router::Guidance {
        router::Guidance {
            timestamp: units::Time::new(1.0),
            heading: units::PlaneAngle::new(heading_degrees.to_radians()),
            distance: units::Length::new(distance),
        }
    }

    fn wheels_in(guidance: Option<router::Guidance>, frame: &router::ReferenceFrame) -> (f32, f32) {
        let control = Context::default()
            .update(
                units::Time::new(1.0),
                guidance,
                frame,
                &mut router::Status::default(),
            )
            .expect("control always produces a command");
        (
            control.motor_drive.left.into(),
            control.motor_drive.right.into(),
        )
    }

    fn wheels(guidance: Option<router::Guidance>) -> (f32, f32) {
        wheels_in(guidance, &unit_frame())
    }

    fn speed(value: units::AngularVelocity) -> f32 {
        value.into()
    }

    /// Local distance at which the approach speed equals `wheel_speed`.
    fn local_distance_for(wheel_speed: f32) -> f32 {
        let wheel_radius = wheel_radius_metres();
        let approach_time: f32 = APPROACH_TIME.into();
        wheel_speed * wheel_radius * approach_time
    }

    #[test]
    fn wheel_radius_is_half_the_diameter_in_metres() {
        assert!((wheel_radius_metres() - 1.25 * 0.0254).abs() < 1e-6);
    }

    #[test]
    fn straight_ahead_drives_both_wheels_at_cruise() {
        let (left, right) = wheels(Some(guidance(0.0)));
        assert!((left - speed(CRUISE_SPEED)).abs() < 1e-5);
        assert!((right - speed(CRUISE_SPEED)).abs() < 1e-5);
    }

    #[test]
    fn approach_slows_with_local_distance_down_to_the_minimum() {
        let (left, right) = wheels(Some(guidance_at(0.0, local_distance_for(1.0))));
        assert!((left - 1.0).abs() < 1e-4 && (right - 1.0).abs() < 1e-4);

        let (left, right) = wheels(Some(guidance_at(0.0, 1e-4)));
        assert!((left - speed(MIN_APPROACH_SPEED)).abs() < 1e-5);
        assert!((right - speed(MIN_APPROACH_SPEED)).abs() < 1e-5);
    }

    #[test]
    fn global_distance_is_converted_to_local_metres() {
        // 20 km of global distance maps onto a 1 m local diagonal.
        let arena = frame(20_000.0, 1.0);
        let global_distance = local_distance_for(1.0) * 20_000.0;

        let (left, right) = wheels_in(Some(guidance_at(0.0, global_distance)), &arena);
        assert!((left - 1.0).abs() < 1e-3 && (right - 1.0).abs() < 1e-3);

        // The same global distance in a one-to-one frame is far away.
        let (left, _) = wheels(Some(guidance_at(0.0, global_distance)));
        assert!((left - speed(CRUISE_SPEED)).abs() < 1e-5);
    }

    #[test]
    fn frame_without_positive_diagonals_stops_the_wheels() {
        let degenerate = router::ReferenceFrame::default();
        assert_eq!(wheels_in(Some(guidance(0.0)), &degenerate), (0.0, 0.0));
    }

    #[test]
    fn positive_heading_turns_right_and_negative_turns_left() {
        let (left, right) = wheels(Some(guidance(10.0)));
        assert!(left > right && right > 0.0, "left={left} right={right}");

        let (mirror_left, mirror_right) = wheels(Some(guidance(-10.0)));
        assert!((mirror_left - right).abs() < 1e-5);
        assert!((mirror_right - left).abs() < 1e-5);
    }

    #[test]
    fn target_abeam_or_behind_pivots_in_place() {
        for heading in [90.0, 120.0, -135.0, 170.0] {
            let (left, right) = wheels(Some(guidance(heading)));
            assert!((left + right).abs() < 1e-5, "heading={heading}");
            assert!(
                (left.abs() - speed(MAX_TURN_SPEED)).abs() < 1e-5,
                "heading={heading}"
            );
            assert_eq!(left > 0.0, heading > 0.0, "heading={heading}");
        }
    }

    #[test]
    fn unwrapped_heading_takes_the_shorter_turn() {
        let (left, right) = wheels(Some(guidance(350.0)));
        let (expected_left, expected_right) = wheels(Some(guidance(-10.0)));
        assert!((left - expected_left).abs() < 1e-4);
        assert!((right - expected_right).abs() < 1e-4);
    }

    #[test]
    fn wheel_speeds_stay_within_limits() {
        let max_wheel = speed(MAX_WHEEL_SPEED);
        for distance in [1e-4, local_distance_for(1.0), 100.0] {
            for step in -180..=180 {
                let (left, right) = wheels(Some(guidance_at(step as f32, distance)));
                assert!(left.abs() <= max_wheel + 1e-5, "heading={step}");
                assert!(right.abs() <= max_wheel + 1e-5, "heading={step}");
            }
        }
    }

    #[test]
    fn no_guidance_stops_the_wheels() {
        assert_eq!(wheels(None), (0.0, 0.0));
    }

    #[test]
    fn invalid_guidance_stops_the_wheels() {
        let mut invalid = guidance(0.0);
        invalid.heading = units::PlaneAngle::new(f32::NAN);
        assert_eq!(wheels(Some(invalid)), (0.0, 0.0));

        let mut invalid = guidance(0.0);
        invalid.distance = units::Length::new(f32::INFINITY);
        assert_eq!(wheels(Some(invalid)), (0.0, 0.0));
    }

    #[test]
    fn control_keeps_the_update_timestamp() {
        let control = Context::default()
            .update(
                units::Time::new(4.5),
                Some(guidance(0.0)),
                &unit_frame(),
                &mut router::Status::default(),
            )
            .unwrap();
        let timestamp: f32 = control.timestamp.into();
        assert_eq!(timestamp, 4.5);
    }
}
