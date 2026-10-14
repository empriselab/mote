use serde::{Deserialize, Serialize};

/// Acceleration measurements along the three axes.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Accel {
    /// Acceleration along the x-axis.
    pub x: units::Acceleration,
    /// Acceleration along the y-axis.
    pub y: units::Acceleration,
    /// Acceleration along the z-axis.
    pub z: units::Acceleration,
}

/// Angular velocity measurements along the three axes.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Gyro {
    /// Angular velocity around the x-axis.
    pub x: units::AngularVelocity,
    /// Angular velocity around the y-axis.
    pub y: units::AngularVelocity,
    /// Angular velocity around the z-axis.
    pub z: units::AngularVelocity,
}

/// Inertial measurement unit data containing acceleration and angular velocity.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Imu {
    /// Timestamp
    pub timestamp: units::Time,
    /// Linear acceleration measurements.
    pub accel: Accel,
    /// Angular velocity measurements.
    pub gyro: Gyro,
}

/// A single measured point in a scan.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct ScanPoint {
    /// Quality value reported for this measurement.
    pub quality: u8,
    /// Angle of the measurement.
    pub angle: units::PlaneAngle,
    /// Distance measured at this angle.
    pub distance: units::Length,
}

/// Lidar point cloud
pub type PointCloud = heapless::vec::Vec<ScanPoint, 100>;

/// Waypoint to navigate to
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Scan {
    /// Timestamp
    pub timestamp: units::Time,
    /// Lidar point cloud
    pub point_cloud: PointCloud,
}

/// Waypoint to navigate to
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Coordinate {
    /// Latitude waypoint position
    pub latitude: units::imperial::Degrees,
    /// Longitude waypoint position
    pub longitude: units::imperial::Degrees,
}

impl Default for Coordinate {
    fn default() -> Self {
        Self {
            latitude: units::imperial::Degrees::new(0.0),
            longitude: units::imperial::Degrees::new(0.0),
        }
    }
}

/// A collection of points captured during a scan.
pub type Route = heapless::vec::Vec<Coordinate, 100>;

/// Maximum serialized route size in bytes.
const MAX_HASH_SIZE: usize = 4 * 1024;

/// Computes a portable 32-bit FNV-1a hash of a route.
pub fn hash_route(route: &Route) -> u32 {
    hash_serialized(route)
}

fn hash_serialized(value: &impl Serialize) -> u32 {
    let mut buffer = [0; MAX_HASH_SIZE];
    let bytes = postcard::to_slice(value, &mut buffer).expect("router data exceeds 4 KiB");
    hash_bytes(bytes)
}

fn hash_bytes(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

/// Guidance describing a target heading and distance.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Guidance {
    /// Timestamp
    pub timestamp: units::Time,
    /// Heading toward the target.
    pub heading: units::PlaneAngle,
    /// Distance to the target.
    pub distance: units::Length,
}

impl Default for Guidance {
    fn default() -> Self {
        Self {
            timestamp: units::Time::new(0.0),
            heading: units::PlaneAngle::new(0.0),
            distance: units::Length::new(0.0),
        }
    }
}

/// Settings including the route and the reference frame.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Reference frame for simulation
    pub reference_frame: ReferenceFrame,
    /// Route for the simulation
    pub route: Route,
}

/// Global center and diagonal scale of the navigation reference frame.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct ReferenceFrame {
    /// Center of the reference frame in global coordinates.
    pub global_center: Coordinate,
    /// Global SW-to-NE diagonal length; the diagonal bisects north and east.
    pub global_diagonal: units::Length,
    /// Local SW-to-NE diagonal length; the diagonal bisects north and east.
    pub local_diagonal: units::Length,
    /// Length of each side wall of a reference structure, from the vertex to an edge.
    pub structure_side_wall_length: units::Length,
}

impl ReferenceFrame {
    /// Computes a portable 32-bit FNV-1a hash of the reference frame.
    pub fn hash(&self) -> u32 {
        hash_serialized(self)
    }
}

impl Default for ReferenceFrame {
    fn default() -> Self {
        Self {
            global_center: Coordinate::default(),
            global_diagonal: units::Length::new(0.0),
            local_diagonal: units::Length::new(0.0),
            structure_side_wall_length: units::Length::new(0.0),
        }
    }
}

/// Position expressed as north/east offsets and a heading.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Position {
    /// Timestamp
    pub timestamp: units::Time,
    /// Northward position offset.
    pub coordinate: Coordinate,
    /// Heading in degrees.
    pub heading: units::imperial::Degrees,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            timestamp: units::Time::new(0.0),
            coordinate: Default::default(),
            heading: units::imperial::Degrees::new(0.0),
        }
    }
}

/// Angular velocity commands for the left and right sides.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Control {
    /// Timestamp
    pub timestamp: units::Time,
    /// Motor drive settings
    pub motor_drive: MotorDrive,
}

/// Telemetry rate selection for pubsub messages
#[derive(Clone, Copy, defmt::Format, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TelemetrySelection {
    /// Reduced telemetry: LiDAR scans are not sent and IMU samples are decimated to about 5 Hz.
    #[default]
    Slow,
    /// Send all telemetry as it is received.
    Fast,
}

impl TelemetrySelection {
    /// IMU samples received for each IMU sample sent in Slow mode (~50 Hz IMU to ~5 Hz).
    pub const SLOW_IMU_DECIMATION: u8 = 10;

    /// Send lidar telemetry
    pub fn is_send_lidar(&self) -> bool {
        *self == Self::Fast
    }
}

/// Selects which IMU samples are forwarded as telemetry for a [`TelemetrySelection`].
#[derive(Clone, Default)]
pub struct ImuTelemetryDecimator {
    samples_since_sent: u8,
}

impl ImuTelemetryDecimator {
    /// Records a received IMU sample and returns whether it should be sent.
    pub fn should_send(&mut self, selection: TelemetrySelection) -> bool {
        match selection {
            TelemetrySelection::Fast => {
                self.samples_since_sent = 0;
                true
            }
            TelemetrySelection::Slow => {
                let send = self.samples_since_sent == 0;
                self.samples_since_sent =
                    (self.samples_since_sent + 1) % TelemetrySelection::SLOW_IMU_DECIMATION;
                send
            }
        }
    }
}

/// Selects which source, if any, is allowed to command the motors.
#[derive(Clone, Copy, defmt::Format, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ControlMode {
    /// No source may command the motors; entering this mode stops them.
    #[default]
    None,
    /// Motor drive commands received from the host are applied to the motors.
    Remote,
    /// Motor drive output from the navigation, guidance, and control pipeline is applied to the motors.
    Auto,
}

/// Direct motor drive command
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct MotorDrive {
    /// left drive speed
    pub left: units::AngularVelocity,
    /// right drive speed
    pub right: units::AngularVelocity,
}

/// Counters used for instrumenting the code
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq, Default)]
pub struct StatusCounters {
    /// number of lidar scans processed
    pub lidar_scan: u32,
    /// number of imu scans processed
    pub imu_sample: u32,
    /// lidar scan fix count
    pub navigation_lidar_scan_fix: u16,
    /// lidar revolutions
    pub navigation_lidar_revolutions: u16,
    /// number of sw corner detections
    pub navigation_sw_corners: u16,
    /// number of ne corner detections
    pub navigation_ne_corners: u16,
    /// position counter
    pub navigation_lidar_position_counter: u16,
    /// fix from corners
    pub navigation_lidar_fix_from_corners: u16,
    /// fix using a recent corner if needed
    pub navigation_lidar_both_corners_with_recent: u16,
    /// dead-reckoning steps since last lidar fix
    pub navigation_dead_reckoning_calcs_since_lidar_fix: u16,
}

/// Update a counter allowing for overflow
pub fn update_counter<T: num_traits::ops::overflowing::OverflowingAdd>(input: &mut T, rhs: T) {
    let (updated_value, _) = input.overflowing_add(&rhs);
    *input = updated_value;
}

/// Debug messages
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub struct Status {
    /// timestamp
    timestamp: units::Time,
    /// increments once for each message sent
    pub sequence: u32,
    /// Log Message
    pub log: heapless::String<64>,
    /// Debug counters
    pub counters: StatusCounters,
    /// Reference frame hash
    pub reference_frame_hash: u32,
    /// Route hash
    pub route_hash: u32,
}

impl Status {
    /// Computes a portable 32-bit FNV-1a hash of the reference frame.
    pub fn hash(&self) -> u32 {
        hash_serialized(self)
    }
}

impl Default for Status {
    fn default() -> Self {
        Self {
            timestamp: units::Time::new(0.0),
            sequence: 0,
            log: heapless::String::new(),
            counters: StatusCounters::default(),
            reference_frame_hash: 0,
            route_hash: 0,
        }
    }
}

impl Status {
    /// Sets a static log message
    pub fn set_log(&mut self, message: &str) {
        match heapless::String::<64>::try_from(message) {
            Ok(value) => {
                self.log = value;
            }
            Err(_) => {
                self.log = heapless::String::<64>::try_from("Bad log message. Too long").unwrap();
            }
        };
    }
}

/// A message published between mote components.
#[derive(Clone, defmt::Format, Serialize, Deserialize, PartialEq)]
pub enum Message {
    /// No message or no available data.
    None,
    /// Request to change the motor control mode.
    ControlMode(ControlMode),
    /// Reset the router
    ResetRoute,
    /// Enable pubsub telemetry
    TelemetrySelection(TelemetrySelection),
    /// Reference frame
    ReferenceFrame(ReferenceFrame),
    /// Motor Drive
    MotorDrive(MotorDrive),
    /// Guidance to a target.
    Guidance(Guidance),
    /// Position estimate.
    Position(Position),
    /// Left and right angular velocity commands.
    Control(Control),
    /// Inertial measurement data.
    Imu(Imu),
    /// A batch of LiDAR scan points.
    LidarScan(Scan),
    /// Guidance Route
    Route(Route),
    /// Debug Message
    Debug(Status),
}

impl alloc::fmt::Debug for Message {
    fn fmt(&self, _formatter: &mut alloc::fmt::Formatter) -> alloc::fmt::Result {
        Ok(())
    }
}

impl Message {
    /// Wraps guidance data in a message.
    pub fn new_from_guidance(value: Guidance) -> Self {
        Self::Guidance(value)
    }

    /// Wraps position data in a message.
    pub fn new_from_position(value: Position) -> Self {
        Self::Position(value)
    }

    /// Wraps control data in a message.
    pub fn new_from_control(value: Control) -> Self {
        Self::Control(value)
    }

    /// Wraps LiDAR scan data in a message.
    pub fn new_from_lidar_scan(value: Scan) -> Self {
        Self::LidarScan(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{ImuTelemetryDecimator, TelemetrySelection, hash_bytes};

    #[test]
    fn fnv1a_matches_known_vectors() {
        assert_eq!(hash_bytes(b""), 0x811c_9dc5);
        assert_eq!(hash_bytes(b"hello"), 0x4f9f_2cab);
    }

    #[test]
    fn slow_telemetry_omits_lidar_and_decimates_imu() {
        assert!(!TelemetrySelection::Slow.is_send_lidar());
        let mut decimator = ImuTelemetryDecimator::default();
        let sent = (0..50)
            .filter(|_| decimator.should_send(TelemetrySelection::Slow))
            .count();
        assert_eq!(sent, 5);
    }

    #[test]
    fn fast_telemetry_sends_everything() {
        assert!(TelemetrySelection::Fast.is_send_lidar());
        let mut decimator = ImuTelemetryDecimator::default();
        assert!((0..50).all(|_| decimator.should_send(TelemetrySelection::Fast)));
    }
}
