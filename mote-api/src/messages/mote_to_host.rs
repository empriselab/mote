//!  Sensor and state data telemetered to the host

use alloc::{boxed::Box, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[cfg(feature = "schemars")]
use schemars::JsonSchema;

// RUNTIME MESSAGES

// Lidar Data
/// A single lidar range reading.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Reading quality/confidence, as reported by the lidar driver.
    pub quality: u8,
    /// Angle of this reading, in radians.
    pub angle_rad: f32,
    /// Measured distance, in millimeters.
    pub distance_mm: f32,
}

// Encoder / Drive Base Data
/// Encoder-derived state of a single drive wheel.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct WheelJointState {
    /// Applied motor effort, as a percentage of maximum.
    pub effort_percent: f32,
    /// Angular velocity, in radians/second.
    pub velocity_rad_per_s: f32,
    /// Cumulative angular position, in radians.
    pub position_rad: f32,
}

/// Encoder-derived state of both drive wheels.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DriveBaseState {
    /// Left wheel state.
    pub left: WheelJointState,
    /// Right wheel state.
    pub right: WheelJointState,
}

// IMU Data
/// A 3-axis IMU reading (used for both acceleration and angular velocity).
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct IMUAxisTriple {
    /// X axis.
    pub x: f32,
    /// Y axis.
    pub y: f32,
    /// Z axis.
    pub z: f32,
}

/// A single IMU sample.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IMUMeasurement {
    /// Linear acceleration.
    pub accel: IMUAxisTriple,
    /// Angular velocity.
    pub gyro: IMUAxisTriple,
}

// CONFIGURATION MESSAGES

/// A WiFi network visible to Mote during a scan.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkConnection {
    /// Network SSID.
    pub ssid: String,
    /// Signal strength (RSSI).
    pub strength: u8,
}

/// Outcome of a single built-in test.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BITResult {
    /// The test hasn't completed yet.
    Waiting,
    /// The test passed.
    Pass,
    /// The test failed.
    Fail,
}

/// A single named built-in test and its outcome.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BIT {
    /// Human-readable test name.
    pub name: String,
    /// Test outcome.
    pub result: BITResult,
}
/// A list of built-in tests for one subsystem.
pub type BITList = Vec<BIT>;

/// Built-in test results, grouped by subsystem.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct BITCollection {
    /// Power subsystem tests.
    pub power: BITList,
    /// WiFi subsystem tests.
    pub wifi: BITList,
    /// Lidar subsystem tests.
    pub lidar: BITList,
    /// IMU subsystem tests.
    pub imu: BITList,
    /// Drive base encoder tests.
    pub encoders: BITList,
}

impl BITCollection {
    /// Const-constructible equivalent of `Default::default()`, for use in
    /// `const`/`static` contexts where `Default::default()` isn't callable.
    pub const fn new() -> Self {
        Self {
            power: Vec::new(),
            wifi: Vec::new(),
            lidar: Vec::new(),
            imu: Vec::new(),
            encoders: Vec::new(),
        }
    }
}

/// Mote's user-assigned device identifier.
pub type UID = String;

/// Why the most recent network connection attempt failed.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ConnectionError {
    /// The join attempt didn't complete within the retry timeout.
    Timeout,
    /// The network refused the connection (wrong password, or the network
    /// rejected the join for any other reason the driver doesn't distinguish).
    AuthOrRefused,
    /// A failure mode not yet modeled as its own variant.
    Other(String),
}

/// Mote's current aggregate state, as telemetered to the host.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct State {
    /// Device identifier.
    pub uid: UID,
    /// Current IP address, if connected to a network.
    pub ip: Option<String>,
    /// Device MAC address.
    pub mac: Option<String>,
    /// Result of the most recent network connection attempt:
    /// `Some(Ok(ssid))` when connected to `ssid`, `Some(Err(reason))` when the
    /// last attempt failed, or `None` when idle or while a connection is in
    /// progress.
    pub current_network_connection: Option<Result<String, ConnectionError>>,
    /// WiFi networks visible in the most recent scan.
    pub available_network_connections: Vec<NetworkConnection>,
    /// Built-in test results.
    pub built_in_test: BITCollection,
}

impl State {
    /// Const-constructible equivalent of `Default::default()`, for use in
    /// `const`/`static` contexts where `Default::default()` isn't callable.
    pub const fn new() -> Self {
        Self {
            uid: UID::new(),
            ip: None,
            mac: None,
            current_network_connection: None,
            available_network_connections: Vec::new(),
            built_in_test: BITCollection::new(),
        }
    }
}

/// A message sent from Mote to the host.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Message {
    /// Liveness check; the host responds with [`Pong`](Message::Pong).
    Ping,
    /// Response to [`Ping`](Message::Ping).
    Pong,
    /// A batch of lidar readings.
    Scan(Vec<Point>),
    /// Drive base wheel state.
    DriveBaseState(DriveBaseState),
    /// An IMU sample.
    IMUMeasurement(IMUMeasurement),
    /// Full aggregate device state.
    State(Box<State>),
}
