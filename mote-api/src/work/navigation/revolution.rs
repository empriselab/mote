use super::{is_finite_angle, is_finite_length, lidar_to_robot, wrap_2pi};

/// Number of one-degree bins used to represent a lidar revolution.
pub const BIN_COUNT: usize = 360;

/// Number of bins past 360 degrees that repeat the start of the revolution.
///
/// A reference structure crossing the 0-degree seam is then contiguous in
/// the extended bins. A structure with 0.2 m legs subtends roughly 40 degrees
/// at half a metre, so a quarter revolution covers it.
pub const OVERLAP_BIN_COUNT: usize = 90;

/// Total number of bins stored per revolution, including the overlap.
pub const EXTENDED_BIN_COUNT: usize = BIN_COUNT + OVERLAP_BIN_COUNT;

/// Minimum number of populated angular bins for a complete revolution.
pub const MIN_BINS: u16 = 150;

/// Maximum number of points exposed by one binned revolution, including overlap bins.
pub const REVOLUTION_POINT_CAPACITY: usize = EXTENDED_BIN_COUNT;

pub(super) const ONE_DEGREE: units::PlaneAngle =
    units::PlaneAngle::new(core::f32::consts::PI / 180.0);
const HALF_BIN: units::PlaneAngle = units::PlaneAngle::new(core::f32::consts::PI / 360.0);

/// A lidar revolution accumulated into uniformly spaced angle bins.
///
/// Bins `BIN_COUNT..EXTENDED_BIN_COUNT` mirror bins `0..OVERLAP_BIN_COUNT`, so
/// structures crossing the 0-degree seam can be detected without reordering.
pub struct Revolution {
    range_m: [units::Length; EXTENDED_BIN_COUNT],
    last_angle: units::PlaneAngle,
    filled: u16,
    start_timestamp: units::Time,
}

impl Revolution {
    pub(super) fn new(start_timestamp: units::Time) -> Self {
        Self {
            range_m: [units::Length::new(0.0); EXTENDED_BIN_COUNT],
            last_angle: units::PlaneAngle::new(0.0),
            filled: 0,
            start_timestamp,
        }
    }

    /// Build a binned revolution from unit-typed lidar samples.
    pub fn from_points(
        start_timestamp: units::Time,
        points: &[(units::PlaneAngle, units::Length)],
    ) -> Self {
        let mut revolution = Self::new(start_timestamp);
        for &(angle, distance) in points {
            if is_finite_angle(angle)
                && is_finite_length(distance)
                && distance > units::Length::new(0.0)
            {
                revolution.insert(wrap_2pi(angle), distance);
            }
        }
        revolution
    }

    pub(super) fn insert(&mut self, angle: units::PlaneAngle, distance: units::Length) {
        let angle = wrap_2pi(angle);
        let bin = angle_bin(angle);
        let previous = self.range_m[bin];
        if previous <= units::Length::new(0.0) {
            self.filled += 1;
            self.range_m[bin] = distance;
        } else if distance < previous {
            self.range_m[bin] = distance;
        }
        if bin < OVERLAP_BIN_COUNT {
            self.range_m[bin + BIN_COUNT] = self.range_m[bin];
        }
        self.last_angle = angle;
    }

    pub(super) const fn last_angle(&self) -> units::PlaneAngle {
        self.last_angle
    }

    /// Number of populated bins in this revolution, excluding overlap bins.
    pub const fn filled_bins(&self) -> u16 {
        self.filled
    }

    /// Timestamp of the first accepted point in this revolution.
    pub const fn start_timestamp(&self) -> units::Time {
        self.start_timestamp
    }

    /// Return the populated bins as robot-frame points in increasing lidar-angle order.
    ///
    /// The sequence continues through the overlap bins, whose indices are
    /// `BIN_COUNT` or greater and repeat the points of the first
    /// [`OVERLAP_BIN_COUNT`] bins.
    pub fn points(&self) -> RevolutionPoints<'_> {
        RevolutionPoints {
            revolution: self,
            next_bin: 0,
        }
    }
}

/// Iterator over the populated points in a [`Revolution`].
pub struct RevolutionPoints<'a> {
    revolution: &'a Revolution,
    next_bin: usize,
}

impl Iterator for RevolutionPoints<'_> {
    type Item = (units::Length, units::Length, units::Length, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while self.next_bin < EXTENDED_BIN_COUNT {
            let bin = self.next_bin;
            self.next_bin += 1;
            let range = self.revolution.range_m[bin];
            if range <= units::Length::new(0.0) {
                continue;
            }

            let angle = bin_angle(bin % BIN_COUNT);
            let (x, y) = lidar_to_robot(angle, range);
            return Some((x, y, range, bin));
        }
        None
    }
}

fn angle_bin(angle: units::PlaneAngle) -> usize {
    let target = wrap_2pi(angle + HALF_BIN);
    let mut lower = units::PlaneAngle::new(0.0);
    for bin in 0..BIN_COUNT {
        let upper = lower + ONE_DEGREE;
        if target < upper {
            return bin;
        }
        lower = upper;
    }
    0
}

fn bin_angle(bin: usize) -> units::PlaneAngle {
    ONE_DEGREE * units::Scalar::new(bin as f32)
}
