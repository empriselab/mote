//! Waypoint guidance.
//!
//! Turns the current position estimate into a [`router::Guidance`] toward the
//! active waypoint of the route. Waypoints are visited in order, starting with
//! the first one in the route.
//!
//! The route is followed approximately rather than precisely. A waypoint is
//! considered reached once the robot is either within a capture radius of it
//! (a fraction of the leg length, so it scales with the route) or has crossed
//! the line through the waypoint perpendicular to the leg being followed. The
//! robot therefore never has to circle back to hit a waypoint exactly, and
//! corners are cut slightly.
//!
//! Offsets use the same flat-earth convention as navigation: one degree of
//! latitude or longitude is [`METRES_PER_DEGREE`]. Distances are in global
//! metres and headings use the compass convention (clockwise positive).

use core::fmt::Write;

use crate::messages::router;
use crate::work::navigation::{METRES_PER_DEGREE, wrap_pi};

/// Fraction of a leg's length within which the leg's end waypoint counts as reached.
const CAPTURE_FRACTION: f32 = 0.25;

#[derive(Default)]
/// Context for doing the guidance work
pub struct Context {
    progress: Option<Progress>,
}

impl Context {
    /// Reset the route progress to the first way point
    pub fn reset_progress(&mut self) {
        self.progress = None;
    }
}

/// Progress along the route currently being followed.
struct Progress {
    /// Hash of the route this progress belongs to; a different route restarts guidance.
    route_hash: u32,
    /// Index of the waypoint being steered to. Equal to the route length once complete.
    target: usize,
    /// Start of the leg ending at `target`: the previous waypoint, or the
    /// position at which guidance began when steering to the first waypoint.
    leg_start: router::Coordinate,
}

impl Context {
    /// Function for updating the data
    ///
    /// Returns `None` when there is nothing to steer to: the route is empty,
    /// the position is invalid, or the final waypoint has been reached.
    pub fn update(
        &mut self,
        timestamp: units::Time,
        position: &router::Position,
        route: &router::Route,
        debug: &mut router::Status,
    ) -> Option<router::Guidance> {
        if route.is_empty() || !is_valid_position(position) {
            return None;
        }

        let route_hash = router::hash_route(route);
        if self
            .progress
            .as_ref()
            .is_none_or(|progress| progress.route_hash != route_hash)
        {
            self.progress = Some(Progress {
                route_hash,
                target: 0,
                leg_start: position.coordinate.clone(),
            });
        }
        let progress = self.progress.as_mut()?;

        while let Some(target) = route.get(progress.target) {
            if !is_reached(&progress.leg_start, target, &position.coordinate) {
                break;
            }
            progress.leg_start = target.clone();
            progress.target += 1;
            log_waypoint_reached(debug, progress.target, route.len());
        }

        let target = route.get(progress.target)?;
        let to_target = Offset::between(&position.coordinate, target);
        let current_heading: units::PlaneAngle = position.heading.into();

        Some(router::Guidance {
            timestamp,
            heading: wrap_pi(to_target.bearing() - current_heading),
            distance: units::Length::new(to_target.length()),
        })
    }
}

/// A waypoint is reached once the robot is inside the capture radius or has
/// passed the perpendicular through the waypoint along the current leg.
fn is_reached(
    leg_start: &router::Coordinate,
    target: &router::Coordinate,
    position: &router::Coordinate,
) -> bool {
    let leg = Offset::between(leg_start, target);
    let past_target = Offset::between(target, position);
    past_target.length() <= CAPTURE_FRACTION * leg.length() || leg.dot(&past_target) >= 0.0
}

fn log_waypoint_reached(debug: &mut router::Status, reached: usize, total: usize) {
    let mut log = heapless::String::<64>::new();
    let _ = if reached == total {
        write!(log, "Guidance: route complete ({total} waypoints)")
    } else {
        write!(log, "Guidance: waypoint {reached} of {total} reached")
    };
    debug.log = log;
}

fn is_valid_position(position: &router::Position) -> bool {
    degrees(position.coordinate.latitude).is_finite()
        && degrees(position.coordinate.longitude).is_finite()
        && degrees(position.heading).is_finite()
}

fn degrees(value: units::imperial::Degrees) -> f32 {
    value.into()
}

/// Flat-earth east/north offset in global metres.
struct Offset {
    east: f32,
    north: f32,
}

impl Offset {
    fn between(from: &router::Coordinate, to: &router::Coordinate) -> Self {
        let metres_per_degree: f32 = METRES_PER_DEGREE.into();
        Self {
            east: (degrees(to.longitude) - degrees(from.longitude)) * metres_per_degree,
            north: (degrees(to.latitude) - degrees(from.latitude)) * metres_per_degree,
        }
    }

    fn length(&self) -> f32 {
        libm::hypotf(self.east, self.north)
    }

    fn dot(&self, other: &Self) -> f32 {
        self.east * other.east + self.north * other.north
    }

    /// Compass bearing of the offset: north is zero and clockwise is positive.
    fn bearing(&self) -> units::PlaneAngle {
        units::PlaneAngle::new(libm::atan2f(self.east, self.north))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::FRAC_PI_2;

    fn coordinate(latitude: f32, longitude: f32) -> router::Coordinate {
        router::Coordinate {
            latitude: units::imperial::Degrees::new(latitude),
            longitude: units::imperial::Degrees::new(longitude),
        }
    }

    fn position(latitude: f32, longitude: f32, heading_degrees: f32) -> router::Position {
        router::Position {
            timestamp: units::Time::new(0.0),
            coordinate: coordinate(latitude, longitude),
            heading: units::imperial::Degrees::new(heading_degrees),
        }
    }

    fn route(points: &[(f32, f32)]) -> router::Route {
        points
            .iter()
            .map(|&(latitude, longitude)| coordinate(latitude, longitude))
            .collect()
    }

    fn heading(guidance: &router::Guidance) -> f32 {
        guidance.heading.into()
    }

    fn distance(guidance: &router::Guidance) -> f32 {
        guidance.distance.into()
    }

    fn metres(degrees: f32) -> f32 {
        let metres_per_degree: f32 = METRES_PER_DEGREE.into();
        degrees * metres_per_degree
    }

    fn update(
        context: &mut Context,
        position: &router::Position,
        route: &router::Route,
    ) -> Option<router::Guidance> {
        context.update(
            units::Time::new(1.0),
            position,
            route,
            &mut router::Status::default(),
        )
    }

    #[test]
    fn empty_route_gives_no_guidance() {
        let mut context = Context::default();
        assert!(update(&mut context, &position(0.0, 0.0, 0.0), &route(&[])).is_none());
    }

    #[test]
    fn starts_with_first_waypoint_even_when_another_is_closer() {
        let mut context = Context::default();
        let route = route(&[(0.0, 1.0), (0.0, -0.1)]);
        let guidance = update(&mut context, &position(0.0, 0.0, 0.0), &route).unwrap();

        assert!((heading(&guidance) - FRAC_PI_2).abs() < 1e-4);
        assert!((distance(&guidance) - metres(1.0)).abs() < 1.0);
    }

    #[test]
    fn heading_is_relative_to_current_heading() {
        let mut context = Context::default();
        let route = route(&[(1.0, 0.0)]);

        let facing_east = update(&mut context, &position(0.0, 0.0, 90.0), &route).unwrap();
        assert!((heading(&facing_east) + FRAC_PI_2).abs() < 1e-4);

        let facing_south_west = update(&mut context, &position(0.0, 0.0, 225.0), &route).unwrap();
        assert!((heading(&facing_south_west) - 3.0 * core::f32::consts::FRAC_PI_4).abs() < 1e-4);
    }

    #[test]
    fn advances_when_near_but_not_at_waypoint() {
        let mut context = Context::default();
        let route = route(&[(1.0, 0.0), (1.0, 1.0)]);
        update(&mut context, &position(0.0, 0.0, 0.0), &route).unwrap();

        // Short of the first waypoint but inside its capture radius.
        let guidance = update(&mut context, &position(0.9, 0.05, 0.0), &route).unwrap();
        assert!(heading(&guidance) > 1.4 && heading(&guidance) < FRAC_PI_2);
    }

    #[test]
    fn advances_after_passing_waypoint_wide() {
        let mut context = Context::default();
        let route = route(&[(1.0, 0.0), (2.0, 0.0)]);
        update(&mut context, &position(0.0, 0.0, 0.0), &route).unwrap();

        // Outside the capture radius, but past the first waypoint along the leg.
        let guidance = update(&mut context, &position(1.1, 0.5, 0.0), &route).unwrap();
        let expected = libm::atan2f(-0.5, 0.9);
        assert!((heading(&guidance) - expected).abs() < 1e-3);
    }

    #[test]
    fn keeps_target_until_reached() {
        let mut context = Context::default();
        let route = route(&[(1.0, 0.0), (1.0, 1.0)]);
        update(&mut context, &position(0.0, 0.0, 0.0), &route).unwrap();

        let guidance = update(&mut context, &position(0.5, 0.0, 0.0), &route).unwrap();
        assert!(heading(&guidance).abs() < 1e-4);
        assert!((distance(&guidance) - metres(0.5)).abs() < 1.0);
    }

    #[test]
    fn completes_after_final_waypoint() {
        let mut context = Context::default();
        let route = route(&[(1.0, 0.0), (2.0, 0.0)]);
        let mut status = router::Status::default();
        update(&mut context, &position(0.0, 0.0, 0.0), &route).unwrap();

        let done = context.update(
            units::Time::new(2.0),
            &position(2.05, 0.0, 0.0),
            &route,
            &mut status,
        );
        assert!(done.is_none());
        assert_eq!(
            status.log.as_str(),
            "Guidance: route complete (2 waypoints)"
        );

        // Stays complete rather than turning back.
        assert!(update(&mut context, &position(1.5, 0.0, 0.0), &route).is_none());
    }

    #[test]
    fn new_route_restarts_from_first_waypoint() {
        let mut context = Context::default();
        let first = route(&[(1.0, 0.0)]);
        update(&mut context, &position(0.0, 0.0, 0.0), &first).unwrap();
        assert!(update(&mut context, &position(1.0, 0.0, 0.0), &first).is_none());

        let second = route(&[(1.0, 1.0), (0.0, 1.0)]);
        let guidance = update(&mut context, &position(1.0, 0.0, 0.0), &second).unwrap();
        assert!((heading(&guidance) - FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn invalid_position_gives_no_guidance() {
        let mut context = Context::default();
        let route = route(&[(1.0, 0.0)]);
        assert!(update(&mut context, &position(f32::NAN, 0.0, 0.0), &route).is_none());
    }
}
