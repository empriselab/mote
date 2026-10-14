//! Position and heading fixes from reference-corner observations.
//!
//! Corner observations are expressed in the robot frame: x points forward,
//! y points left, and their bearing is counter-clockwise from forward. The
//! local navigation frame is east/north, while headings use the compass
//! convention (north is zero and clockwise is positive).
//!
//! Only the vertex locations are trusted. The tilt of each structure is not
//! controlled, so the direction of its walls carries no heading information.

use super::{
    CornerKind, CornerObservation, LocalPose, LocalPosition, MAX_FIX_DISAGREEMENT,
    baseline_bearing, is_finite_angle, is_finite_length, is_finite_scalar, northeast_local,
    wrap_2pi,
};
use crate::messages::router;

const EPSILON: units::Scalar = units::Scalar::new(1.0e-6);
const ZERO_LENGTH: units::Length = units::Length::new(0.0);
const HALF: units::Scalar = units::Scalar::new(0.5);

#[derive(Clone, Copy)]
struct ObservationVector {
    x: units::Length,
    y: units::Length,
}

#[derive(Clone, Copy)]
struct SelectedObservation {
    observation: CornerObservation,
    vector: ObservationVector,
}

/// Recover a local position and compass heading from corner observations.
///
/// With both corners visible, the heading comes from the direction of the
/// measured southwest-to-northeast vertex vector and the position from the
/// two vertex offsets. A single corner fixes only position, so it needs a
/// `prior_heading` (for example the gyro-propagated heading); without one it
/// yields no fix.
pub fn fix_from_corners(
    frame: &router::ReferenceFrame,
    observations: &[CornerObservation],
    prior_heading: Option<units::PlaneAngle>,
) -> Option<LocalPose> {
    let selected = select_observations(observations);
    let southwest = selected[0];
    let northeast = selected[1];

    match (southwest, northeast) {
        (Some(southwest), Some(northeast)) => two_corner_fix(frame, southwest, northeast),
        (Some(observation), None) | (None, Some(observation)) => {
            one_corner_fix(frame, observation, prior_heading?)
        }
        (None, None) => None,
    }
}

fn select_observations(observations: &[CornerObservation]) -> [Option<SelectedObservation>; 2] {
    let mut selected: [Option<SelectedObservation>; 2] = [None, None];

    for &observation in observations {
        if !is_finite_scalar(observation.score)
            || !is_finite_length(observation.range_m)
            || !is_finite_angle(observation.bearing_rad)
            || observation.score < units::Scalar::new(0.0)
        {
            continue;
        }
        let Some(vector) = observation_vector(&observation) else {
            continue;
        };

        let slot = match observation.kind {
            CornerKind::Southwest => &mut selected[0],
            CornerKind::Northeast => &mut selected[1],
        };
        let replace = slot
            .as_ref()
            .map(|current| observation.score > current.observation.score)
            .unwrap_or(true);
        if replace {
            *slot = Some(SelectedObservation {
                observation,
                vector,
            });
        }
    }

    selected
}

fn one_corner_fix(
    frame: &router::ReferenceFrame,
    selected: SelectedObservation,
    heading: units::PlaneAngle,
) -> Option<LocalPose> {
    if !is_finite_angle(heading) {
        return None;
    }
    let heading = wrap_2pi(heading);
    Some(LocalPose {
        position: position_from_observation(frame, selected, heading),
        heading,
    })
}

fn two_corner_fix(
    frame: &router::ReferenceFrame,
    southwest: SelectedObservation,
    northeast: SelectedObservation,
) -> Option<LocalPose> {
    let baseline = northeast_local(frame);
    if distance(baseline.east, baseline.north) <= ZERO_LENGTH {
        defmt::info!("Reject for bad distance");
        return None;
    }

    let sw_to_ne_robot = ObservationVector {
        x: northeast.vector.x - southwest.vector.x,
        y: northeast.vector.y - southwest.vector.y,
    };
    let sw_to_ne_length = distance(sw_to_ne_robot.x, sw_to_ne_robot.y);
    if sw_to_ne_length <= ZERO_LENGTH {
        defmt::info!("Reject for bad distance for SW/NE");
        return None;
    }
    let robot_bearing = angle_from_atan2(
        sw_to_ne_robot.y / sw_to_ne_length,
        sw_to_ne_robot.x / sw_to_ne_length,
    );
    let heading = wrap_2pi(baseline_bearing(frame) + robot_bearing);

    // With a shared heading the two positions differ only along the baseline,
    // by the difference between the measured and expected vertex separation.
    let southwest_position = position_from_observation(frame, southwest, heading);
    let northeast_position = position_from_observation(frame, northeast, heading);
    let disagreement = distance(
        southwest_position.east - northeast_position.east,
        southwest_position.north - northeast_position.north,
    );
    if disagreement > MAX_FIX_DISAGREEMENT {
        defmt::info!("Max fix disagreement - distance {}", disagreement);
        return None;
    }

    let southwest_score = max_scalar(southwest.observation.score, units::Scalar::new(0.0));
    let northeast_score = max_scalar(northeast.observation.score, units::Scalar::new(0.0));
    let total_score = southwest_score + northeast_score;
    let (southwest_weight, northeast_weight) = if total_score > EPSILON {
        (southwest_score / total_score, northeast_score / total_score)
    } else {
        (HALF, HALF)
    };
    let position = LocalPosition {
        east: southwest_position.east * southwest_weight
            + northeast_position.east * northeast_weight,
        north: southwest_position.north * southwest_weight
            + northeast_position.north * northeast_weight,
    };

    Some(LocalPose { position, heading })
}

fn position_from_observation(
    frame: &router::ReferenceFrame,
    selected: SelectedObservation,
    heading: units::PlaneAngle,
) -> LocalPosition {
    let corner = corner_position(frame, selected.observation.kind);
    let offset = rotate_robot_vector(selected.vector, heading);
    LocalPosition {
        east: corner.east - offset.x,
        north: corner.north - offset.y,
    }
}

fn corner_position(frame: &router::ReferenceFrame, kind: CornerKind) -> LocalPosition {
    let northeast = northeast_local(frame);
    match kind {
        CornerKind::Southwest => LocalPosition {
            east: northeast.east * units::Scalar::new(-1.0),
            north: northeast.north * units::Scalar::new(-1.0),
        },
        CornerKind::Northeast => northeast,
    }
}

/// Rotate a robot-frame (forward, left) vector into local (east, north)
/// coordinates using a compass heading.
fn rotate_robot_vector(vector: ObservationVector, heading: units::PlaneAngle) -> ObservationVector {
    ObservationVector {
        x: vector.x * heading.sin() - vector.y * heading.cos(),
        y: vector.x * heading.cos() + vector.y * heading.sin(),
    }
}

fn observation_vector(observation: &CornerObservation) -> Option<ObservationVector> {
    let range = observation.range_m;
    if range <= ZERO_LENGTH {
        return None;
    }
    Some(ObservationVector {
        x: range * observation.bearing_rad.cos(),
        y: range * observation.bearing_rad.sin(),
    })
}

fn distance(x: units::Length, y: units::Length) -> units::Length {
    (x * x + y * y).sqrt()
}

fn angle_from_atan2(y: units::Scalar, x: units::Scalar) -> units::PlaneAngle {
    units::PlaneAngle::new(1.0) * y.atan2(x)
}

fn max_scalar(first: units::Scalar, second: units::Scalar) -> units::Scalar {
    if first > second { first } else { second }
}
