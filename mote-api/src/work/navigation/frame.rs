//! Coordinate and frame-convention helpers for navigation.
//!
//! The navigation frame is a flat local tangent plane centered on the global
//! reference coordinate. Its x axis points east and its y axis points north.
use crate::messages::router;

/// Metres in one degree in the flat-earth frame used by navigation.
pub const METRES_PER_DEGREE: units::Scalar = units::Scalar::new(111_319.5);

const TWO_PI: units::PlaneAngle = units::PlaneAngle::new(2.0 * core::f32::consts::PI);
const PI: units::PlaneAngle = units::PlaneAngle::new(core::f32::consts::PI);

/// Local tangent-plane position in lidar metres, with the frame center as origin.
#[derive(Clone, Copy)]
pub struct LocalPosition {
    /// Eastward offset from the frame center, in lidar metres.
    pub east: units::Length,
    /// Northward offset from the frame center, in lidar metres.
    pub north: units::Length,
}

impl Default for LocalPosition {
    fn default() -> Self {
        Self {
            east: units::Length::new(0.0),
            north: units::Length::new(0.0),
        }
    }
}

/// Local pose: a position and a compass-convention heading.
#[derive(Clone, Copy)]
pub struct LocalPose {
    /// Local position in lidar metres.
    pub position: LocalPosition,
    /// Compass-convention heading, in radians.
    pub heading: units::PlaneAngle,
}

impl Default for LocalPose {
    fn default() -> Self {
        Self {
            position: LocalPosition::default(),
            heading: units::PlaneAngle::new(0.0),
        }
    }
}

/// Convert a global coordinate to local lidar metres relative to the frame center.
pub fn to_local(frame: &router::ReferenceFrame, coordinate: &router::Coordinate) -> LocalPosition {
    let (east_world, north_world) = world_offset(frame, coordinate);
    let diagonal_scale = diagonal_scale(frame);

    LocalPosition {
        east: east_world * diagonal_scale,
        north: north_world * diagonal_scale,
    }
}

/// Convert a local lidar-metre position back to the global flat-earth frame.
pub fn to_global(frame: &router::ReferenceFrame, local: LocalPosition) -> router::Coordinate {
    let diagonal_scale = diagonal_scale(frame);
    let (east_world, north_world) = if diagonal_scale > units::Scalar::new(0.0) {
        (local.east / diagonal_scale, local.north / diagonal_scale)
    } else {
        (units::Length::new(0.0), units::Length::new(0.0))
    };
    let metres_per_degree = units::Length::new(1.0) * METRES_PER_DEGREE;
    let east_degrees = east_world / metres_per_degree;
    let north_degrees = north_world / metres_per_degree;

    router::Coordinate {
        latitude: units::imperial::Degrees::new(1.0)
            * (degrees_as_scalar(frame.global_center.latitude) + north_degrees),
        longitude: units::imperial::Degrees::new(1.0)
            * (degrees_as_scalar(frame.global_center.longitude) + east_degrees),
    }
}

fn world_offset(
    frame: &router::ReferenceFrame,
    coordinate: &router::Coordinate,
) -> (units::Length, units::Length) {
    let latitude_delta =
        degrees_as_scalar(coordinate.latitude) - degrees_as_scalar(frame.global_center.latitude);
    let longitude_delta =
        degrees_as_scalar(coordinate.longitude) - degrees_as_scalar(frame.global_center.longitude);
    let metres_per_degree = units::Length::new(1.0) * METRES_PER_DEGREE;

    (
        metres_per_degree * longitude_delta,
        metres_per_degree * latitude_delta,
    )
}

/// Local lidar metres per global metre, or zero when either diagonal is not positive.
///
/// The conversion is a uniform scale, so headings are the same in both frames.
pub fn diagonal_scale(frame: &router::ReferenceFrame) -> units::Scalar {
    if frame.global_diagonal > units::Length::new(0.0)
        && frame.local_diagonal > units::Length::new(0.0)
    {
        frame.local_diagonal / frame.global_diagonal
    } else {
        units::Scalar::new(0.0)
    }
}

/// Return the NE vertex offset from the frame center in local lidar metres.
pub fn northeast_local(frame: &router::ReferenceFrame) -> LocalPosition {
    let half_diagonal_component =
        frame.local_diagonal * units::Scalar::new(0.5 * core::f32::consts::FRAC_1_SQRT_2);
    LocalPosition {
        east: half_diagonal_component,
        north: half_diagonal_component,
    }
}

/// Return the compass bearing from the SW vertex to the NE vertex.
pub fn baseline_bearing(frame: &router::ReferenceFrame) -> units::PlaneAngle {
    let baseline = northeast_local(frame);
    let east = baseline.east / units::Length::new(1.0);
    let north = baseline.north / units::Length::new(1.0);
    wrap_2pi(angle_from_scalar(east.atan2(north)))
}

/// Convert a lidar sample to robot-frame coordinates.
///
/// Lidar angles are zero-forward and clockwise-positive. Robot coordinates
/// are x-forward and y-left, so the y component has the opposite sign.
pub fn lidar_to_robot(
    angle: units::PlaneAngle,
    distance: units::Length,
) -> (units::Length, units::Length) {
    let x = distance * angle.cos();
    let y = distance * angle.sin() * units::Scalar::new(-1.0);
    (x, y)
}

/// Wrap an angle to (-π, π].
pub fn wrap_pi(angle: units::PlaneAngle) -> units::PlaneAngle {
    let wrapped = wrap_2pi(angle);
    if wrapped > PI {
        wrapped - TWO_PI
    } else {
        wrapped
    }
}

/// Wrap an angle to [0, 2π).
pub fn wrap_2pi(angle: units::PlaneAngle) -> units::PlaneAngle {
    let mut wrapped = angle;
    while wrapped >= TWO_PI {
        wrapped -= TWO_PI;
    }
    while wrapped < units::PlaneAngle::new(0.0) {
        wrapped += TWO_PI;
    }
    wrapped
}

/// Convert a radian compass heading to normalized compass degrees.
pub fn heading_to_degrees(angle: units::PlaneAngle) -> units::imperial::Degrees {
    units::imperial::Degrees::from(wrap_2pi(angle))
}

fn degrees_as_scalar(degrees: units::imperial::Degrees) -> units::Scalar {
    degrees / units::imperial::Degrees::new(1.0)
}

fn angle_from_scalar(value: units::Scalar) -> units::PlaneAngle {
    units::PlaneAngle::new(1.0) * value
}
