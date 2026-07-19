//!  Command messages sent to Mote

use alloc::string::String;
use serde::{Deserialize, Serialize};

#[cfg(feature = "schemars")]
use schemars::JsonSchema;

// CONFIGURATION MESSAGES

/// Requests Mote join a WiFi network with the given credentials.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SetNetworkConnectionConfig {
    /// Network SSID to join.
    pub ssid: String,
    /// Network password. Empty for an open network.
    pub password: String,
}

/// Sets Mote's user-assigned device identifier.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SetUid {
    /// The new device identifier.
    pub uid: String,
}

// RUNTIME MESSAGES

/// Commands the drive base's left and right wheel velocities.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SetDriveBaseVelocity {
    /// Left wheel angular velocity, in radians/second.
    pub left_velocity_rad: f32,
    /// Right wheel angular velocity, in radians/second.
    pub right_velocity_rad: f32,
}

/// A message sent from the host to Mote.
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Message {
    /// Liveness check; Mote responds with [`Pong`](Message::Pong).
    Ping,
    /// Response to [`Ping`](Message::Ping).
    Pong,
    /// Requests Mote scan for nearby WiFi networks.
    RequestNetworkScan,
    /// Requests Mote join a WiFi network.
    SetNetworkConnectionConfig(SetNetworkConnectionConfig),
    /// Sets Mote's device identifier.
    SetUid(SetUid),
    /// Commands the drive base's wheel velocities.
    SetDriveBaseVelocity(SetDriveBaseVelocity),
}
