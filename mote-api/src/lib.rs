#![no_std]

//! Messages used by Mote for firmware <--> host communication

// I'd prefer to move away from alloc, but it's here for now.
extern crate alloc;
use core::marker::PhantomData;

use alloc::{collections::vec_deque::VecDeque, vec::Vec};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub mod messages;

use crate::messages::{host_to_mote, mote_to_host};

/// Which side of the mote/host link a piece of code represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Mote,
    Host,
}

impl Role {
    const fn other(self) -> Role {
        match self {
            Role::Mote => Role::Host,
            Role::Host => Role::Mote,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Role::Mote => "mote",
            Role::Host => "host",
        }
    }
}

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Identifies which side of the mote/host link decodes a given message type.
pub trait MessageRole {
    const RECEIVER: Role;
}

impl MessageRole for mote_to_host::Message {
    const RECEIVER: Role = Role::Host;
}

impl MessageRole for host_to_mote::Message {
    const RECEIVER: Role = Role::Mote;
}

/// The number of raw (non-bitcode, non-serde) bytes reserved at the start of every
/// message frame for the version header. This layout is a permanent wire-format
/// invariant: it must always be parseable via plain byte slicing alone, independent
/// of any future change to `bitcode`'s encoding or to the `Message` enums, so that a
/// version mismatch can always be detected and reported even across breaking changes
/// to the rest of the message format.
const VERSION_HEADER_LEN: usize = 6;

/// The mote-api crate version embedded in every message header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    /// The version of the mote-api crate this binary was compiled against.
    pub const LOCAL: Version = Version {
        major: parse_u16(env!("CARGO_PKG_VERSION_MAJOR")),
        minor: parse_u16(env!("CARGO_PKG_VERSION_MINOR")),
        patch: parse_u16(env!("CARGO_PKG_VERSION_PATCH")),
    };

    const fn to_wire_bytes(self) -> [u8; VERSION_HEADER_LEN] {
        let [a0, a1] = self.major.to_le_bytes();
        let [b0, b1] = self.minor.to_le_bytes();
        let [c0, c1] = self.patch.to_le_bytes();
        [a0, a1, b0, b1, c0, c1]
    }

    fn from_wire_bytes(bytes: [u8; VERSION_HEADER_LEN]) -> Self {
        Self {
            major: u16::from_le_bytes([bytes[0], bytes[1]]),
            minor: u16::from_le_bytes([bytes[2], bytes[3]]),
            patch: u16::from_le_bytes([bytes[4], bytes[5]]),
        }
    }

    /// The (major, minor, patch) key that must match exactly for two versions to be
    /// considered wire-compatible, per semver's caret-compatibility rules:
    /// - major >= 1: only `major` is breaking (`^1.2.3` allows any `1.x.y`)
    /// - major == 0, minor >= 1: `minor` is breaking (`^0.2.3` allows any `0.2.y`)
    /// - major == 0, minor == 0: `patch` is breaking (`^0.0.3` allows only `0.0.3`)
    const fn breaking_key(self) -> (u16, u16, u16) {
        if self.major != 0 {
            (self.major, 0, 0)
        } else if self.minor != 0 {
            (0, self.minor, 0)
        } else {
            (0, 0, self.patch)
        }
    }

    fn is_compatible_with(self, other: Version) -> bool {
        self.breaking_key() == other.breaking_key()
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parses a decimal digit string (as produced by Cargo's `CARGO_PKG_VERSION_*` env
/// vars) into a `u16`, at compile time.
const fn parse_u16(s: &str) -> u16 {
    let bytes = s.as_bytes();
    let mut value: u16 = 0;
    let mut i = 0;
    while i < bytes.len() {
        assert!(
            bytes[i].is_ascii_digit(),
            "CARGO_PKG_VERSION component must be decimal digits"
        );
        value = value * 10 + (bytes[i] - b'0') as u16;
        i += 1;
    }
    value
}

/// Error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Bitcode ser/de failed")]
    BitCodeError(#[from] bitcode::Error),
    #[error("Cobs pack/unpack failed")]
    CobsError(corncobs::CobsError),
    #[error("Message frame too short to contain a version header")]
    MalformedHeader,
    #[error(
        "mote-api version mismatch: {local_role} is v{local}, {remote_role} is v{remote} — update {behind} to a compatible version"
    )]
    VersionMismatch {
        local: Version,
        remote: Version,
        local_role: Role,
        remote_role: Role,
        behind: Role,
    },
}

impl From<corncobs::CobsError> for Error {
    fn from(value: corncobs::CobsError) -> Self {
        Self::CobsError(value)
    }
}

/// Implements encoding of message types.
fn to_slice<M>(message: &M) -> Result<Vec<u8>, Error>
where
    M: Serialize + ?Sized,
{
    let body = bitcode::serialize(message)?;
    let mut plain_buf: Vec<u8> = Vec::with_capacity(VERSION_HEADER_LEN + body.len());
    plain_buf.extend_from_slice(&Version::LOCAL.to_wire_bytes());
    plain_buf.extend_from_slice(&body);

    let encoded_size = corncobs::max_encoded_len(plain_buf.len());
    let mut cobs_buff: Vec<u8> = Vec::with_capacity(encoded_size);
    cobs_buff.resize(encoded_size, 10);
    let encoded_size = corncobs::encode_buf(&plain_buf, &mut cobs_buff);
    cobs_buff.truncate(encoded_size);

    Ok(cobs_buff)
}

/// Implements decoding of message types.
fn from_bytes<M>(bytes: &[u8]) -> Result<M, Error>
where
    M: DeserializeOwned + MessageRole,
{
    let mut cobs_buff: Vec<u8> = Vec::with_capacity(bytes.len());
    cobs_buff.resize(bytes.len(), 10);
    let decoded_size = corncobs::decode_buf(bytes, &mut cobs_buff)?;
    cobs_buff.truncate(decoded_size);

    if cobs_buff.len() < VERSION_HEADER_LEN {
        return Err(Error::MalformedHeader);
    }
    let (header, body) = cobs_buff.split_at(VERSION_HEADER_LEN);
    let remote = Version::from_wire_bytes(
        header
            .try_into()
            .expect("split_at guarantees len == VERSION_HEADER_LEN"),
    );

    if !Version::LOCAL.is_compatible_with(remote) {
        let local_role = M::RECEIVER;
        let remote_role = local_role.other();
        let behind = if Version::LOCAL.breaking_key() < remote.breaking_key() {
            local_role
        } else {
            remote_role
        };
        return Err(Error::VersionMismatch {
            local: Version::LOCAL,
            remote,
            local_role,
            remote_role,
            behind,
        });
    }

    Ok(bitcode::deserialize::<M>(body)?)
}

// Sets the capacity for the deserialization ringbuffer
const MAX_MESSAGE_LENGTH: usize = 5000;

/// Bidirectional SansIO communication link betweek mote and the host.
///
/// You probably do not want to directly construct this. Instead, use the type aliases:
/// MoteLink (use on host)
/// HostLink (use on mote)
/// MoteConfigLink
/// HostConfigLink
pub struct MoteComms<const MTU: usize, I, O>
where
    I: DeserializeOwned, // Input type
    O: Serialize,        // Output type
{
    buffered_transmits: VecDeque<Vec<u8>>,
    deserialization_buffer: VecDeque<u8>,

    in_type: PhantomData<I>,
    out_type: PhantomData<O>,
}
impl<const MTU: usize, I, O> Default for MoteComms<MTU, I, O>
where
    I: for<'de> Deserialize<'de>, // Input type
    O: Serialize,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const MTU: usize, I, O> MoteComms<MTU, I, O>
where
    I: for<'de> Deserialize<'de>, // Input type
    O: Serialize,                 // Output type
{
    /// Generate a new link
    pub fn new() -> Self {
        Self {
            buffered_transmits: VecDeque::new(),
            deserialization_buffer: VecDeque::new(),
            in_type: PhantomData,
            out_type: PhantomData,
        }
    }

    /// Queue a message to be sent
    pub fn send(&mut self, message: O) -> Result<(), Error> {
        let encoded_bytes: Vec<u8> = to_slice(&message)?;

        // Break message into packets given the MTU
        for chunk in encoded_bytes.chunks(MTU) {
            self.buffered_transmits.push_back(Vec::from(chunk));
        }

        Ok(())
    }

    /// Get the next packet to be sent
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.buffered_transmits.pop_front()
    }

    /// Receive a message from raw bytes
    pub fn handle_receive(&mut self, packet: &[u8]) {
        // Push the received bytes into the serialization buffer, potentially dropping the first
        // value if the buffer is full
        packet.iter().for_each(|byte| {
            self.deserialization_buffer.push_back(*byte);
            if self.deserialization_buffer.len() > MAX_MESSAGE_LENGTH {
                self.deserialization_buffer.pop_front();
            }
        });
    }

    /// Poll for new messages in the recv buffer
    pub fn poll_receive(&mut self) -> Result<Option<I>, Error>
    where
        I: MessageRole,
    {
        if let Some(end) = self.deserialization_buffer.iter().position(|&x| x == 0) {
            let linear_buf: Vec<u8> = self.deserialization_buffer.drain(0..=end).collect();
            match from_bytes::<I>(&linear_buf) {
                Ok(msg) => Ok(Some(msg)),
                Err(Error::CobsError(corncobs::CobsError::Truncated)) => {
                    // We checked for this in the if above, so it shouldn't happen.
                    // But it isn't an error.
                    Ok(None)
                }
                Err(other) => Err(other),
            }
        } else {
            // No end byte = no message
            Ok(None)
        }
    }
}

/// Used by the host to send commands to and receive data from Mote
pub type MoteLink = MoteComms<
    1400, // UDP MTU(ish)
    mote_to_host::Message,
    host_to_mote::Message,
>;

/// Used by Mote to send data to and receive commands from the host
pub type HostLink = MoteComms<
    1400, // UDP MTU(ish)
    host_to_mote::Message,
    mote_to_host::Message,
>;

/// Used by the host to send commands to and receive data from Mote
pub type MoteConfigLink = MoteComms<
    64, // Serial MTU
    mote_to_host::Message,
    host_to_mote::Message,
>;

/// Used by Mote to send data to and receive commands from the host
pub type HostConfigLink = MoteComms<
    64, // Serial MTU
    host_to_mote::Message,
    mote_to_host::Message,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{boxed::Box, string::String, vec};

    // Returns all mote_to_host message variants including heap-allocated ones.
    fn all_mote_messages() -> Vec<mote_to_host::Message> {
        vec![
            mote_to_host::Message::Ping,
            mote_to_host::Message::Pong,
            mote_to_host::Message::Scan(vec![
                mote_to_host::Point {
                    quality: 255,
                    angle_rad: 1.5707,
                    distance_mm: 500.0,
                },
                mote_to_host::Point {
                    quality: 0,
                    angle_rad: 0.0,
                    distance_mm: 0.0,
                },
            ]),
            mote_to_host::Message::State(Box::new(mote_to_host::State {
                uid: String::from("mote-test"),
                ip: Some(String::from("192.168.1.100")),
                mac: Some(String::from("aa:bb:cc:dd:ee:ff")),
                current_network_connection: Some(Ok(String::from("MyWifi"))),
                available_network_connections: vec![mote_to_host::NetworkConnection {
                    ssid: String::from("MyWifi"),
                    strength: 80,
                }],
                built_in_test: mote_to_host::BITCollection {
                    power: vec![mote_to_host::BIT {
                        name: String::from("battery"),
                        result: mote_to_host::BITResult::Pass,
                    }],
                    wifi: vec![],
                    lidar: vec![mote_to_host::BIT {
                        name: String::from("lidar_init"),
                        result: mote_to_host::BITResult::Waiting,
                    }],
                    imu: vec![],
                    encoders: vec![mote_to_host::BIT {
                        name: String::from("left_enc"),
                        result: mote_to_host::BITResult::Fail,
                    }],
                },
            })),
        ]
    }

    // Returns all host_to_mote message variants.
    fn all_host_messages() -> Vec<host_to_mote::Message> {
        vec![
            host_to_mote::Message::Ping,
            host_to_mote::Message::Pong,
            host_to_mote::Message::RequestNetworkScan,
            host_to_mote::Message::SetNetworkConnectionConfig(
                host_to_mote::SetNetworkConnectionConfig {
                    ssid: String::from("MyWifi"),
                    password: String::from("hunter2"),
                },
            ),
            host_to_mote::Message::SetUID(host_to_mote::SetUID {
                uid: String::from("mote-abc"),
            }),
        ]
    }

    // --- encode / decode ---

    #[test]
    fn test_encode_decode_failed_connection() -> Result<(), Error> {
        let reasons = [
            String::from("timed out"),
            String::from(
                "Failed to join the network (incorrect password or the network refused the connection)",
            ),
        ];
        for reason in reasons {
            let mut state = mote_to_host::State::default();
            state.current_network_connection = Some(Err(reason));
            let msg = mote_to_host::Message::State(alloc::boxed::Box::new(state));

            // Direct codec round-trip.
            let recv: mote_to_host::Message = from_bytes(&to_slice(&msg)?)?;
            assert_eq!(msg, recv);

            // Full serial-link round-trip (mote -> host) with MTU-64 fragmentation.
            let mut host_l = HostConfigLink::new();
            host_l.send(msg.clone())?;
            let mut mote_l = MoteConfigLink::new();
            while let Some(payload) = host_l.poll_transmit() {
                mote_l.handle_receive(&payload);
            }
            assert_eq!(mote_l.poll_receive()?.unwrap(), msg);
        }
        Ok(())
    }

    #[test]
    fn test_encode_decode_all_variants() -> Result<(), Error> {
        for msg in all_mote_messages() {
            let bytes = to_slice(&msg)?;
            let recv: mote_to_host::Message = from_bytes(&bytes)?;
            assert_eq!(msg, recv);
        }
        for msg in all_host_messages() {
            let bytes = to_slice(&msg)?;
            let recv: host_to_mote::Message = from_bytes(&bytes)?;
            assert_eq!(msg, recv);
        }
        Ok(())
    }

    // --- poll_transmit / poll_receive on empty state ---

    #[test]
    fn test_poll_transmit_empty() {
        let mut link = MoteLink::new();
        assert!(link.poll_transmit().is_none());
    }

    #[test]
    fn test_poll_receive_empty() -> Result<(), Error> {
        let mut link = MoteLink::new();
        assert!(link.poll_receive()?.is_none());
        Ok(())
    }

    // --- Default::default() ---

    #[test]
    fn test_default() -> Result<(), Error> {
        let mut link: MoteLink = Default::default();
        link.send(host_to_mote::Message::Ping)?;
        assert!(link.poll_transmit().is_some());
        Ok(())
    }

    // --- config link round-trips (all variants) ---

    #[test]
    fn test_config_links() -> Result<(), Error> {
        for msg in all_mote_messages() {
            let mut host_l = HostConfigLink::new();
            host_l.send(msg.clone())?;
            let mut mote_l = MoteConfigLink::new();
            while let Some(payload) = host_l.poll_transmit() {
                mote_l.handle_receive(&payload);
            }
            assert_eq!(mote_l.poll_receive()?.unwrap(), msg);
        }

        for msg in all_host_messages() {
            let mut mote_l = MoteConfigLink::new();
            mote_l.send(msg.clone())?;
            let mut host_l = HostConfigLink::new();
            while let Some(payload) = mote_l.poll_transmit() {
                host_l.handle_receive(&payload);
            }
            assert_eq!(host_l.poll_receive()?.unwrap(), msg);
        }
        Ok(())
    }

    // --- UDP link round-trips (all variants) ---

    #[test]
    fn test_udp_links() -> Result<(), Error> {
        for msg in all_mote_messages() {
            let mut host_l = HostLink::new();
            host_l.send(msg.clone())?;
            let mut mote_l = MoteLink::new();
            while let Some(payload) = host_l.poll_transmit() {
                mote_l.handle_receive(&payload);
            }
            assert_eq!(mote_l.poll_receive()?.unwrap(), msg);
        }

        for msg in all_host_messages() {
            let mut mote_l = MoteLink::new();
            mote_l.send(msg.clone())?;
            let mut host_l = HostLink::new();
            while let Some(payload) = mote_l.poll_transmit() {
                host_l.handle_receive(&payload);
            }
            assert_eq!(host_l.poll_receive()?.unwrap(), msg);
        }
        Ok(())
    }

    // --- Fragmentation: large message split across MTU=64 packets ---

    #[test]
    fn test_fragmentation() -> Result<(), Error> {
        let scan = mote_to_host::Message::Scan(
            (0..100u8)
                .map(|i| mote_to_host::Point {
                    quality: i,
                    angle_rad: i as f32 * 0.01,
                    distance_mm: i as f32 * 10.0,
                })
                .collect(),
        );

        let mut host_l = HostConfigLink::new(); // MTU = 64
        host_l.send(scan.clone())?;

        let mut packet_count = 0usize;
        let mut mote_l = MoteConfigLink::new();
        while let Some(payload) = host_l.poll_transmit() {
            assert!(payload.len() <= 64, "packet exceeded MTU");
            mote_l.handle_receive(&payload);
            packet_count += 1;
        }
        assert!(
            packet_count > 1,
            "expected fragmentation into multiple packets"
        );

        assert_eq!(mote_l.poll_receive()?.unwrap(), scan);
        Ok(())
    }

    // --- Multiple messages received in order ---

    #[test]
    fn test_multiple_messages_in_order() -> Result<(), Error> {
        let messages = [
            host_to_mote::Message::Ping,
            host_to_mote::Message::RequestNetworkScan,
            host_to_mote::Message::Pong,
        ];
        let mut mote_l = MoteConfigLink::new();
        let mut host_l = HostConfigLink::new();

        for msg in &messages {
            mote_l.send(msg.clone())?;
        }
        while let Some(payload) = mote_l.poll_transmit() {
            host_l.handle_receive(&payload);
        }
        for expected in &messages {
            assert_eq!(&host_l.poll_receive()?.unwrap(), expected);
        }
        assert!(host_l.poll_receive()?.is_none());
        Ok(())
    }

    // --- Bad data in the receive buffer ---

    #[test]
    fn test_truncated_cobs_produces_no_message() -> Result<(), Error> {
        let mut link = MoteLink::new();
        // First byte 0xFF tells COBS to skip 254 more bytes, but the packet ends
        // after three bytes — corncobs returns Truncated, which maps to Ok(None).
        link.handle_receive(&[0xFF, 0xFE, 0xFD, 0x00]);
        assert!(link.poll_receive()?.is_none());
        Ok(())
    }

    #[test]
    fn test_empty_cobs_payload_returns_error() {
        let mut link = MoteLink::new();
        // [0x01, 0x00] is a valid COBS frame (overhead byte 0x01 = no data, then
        // terminator), but the empty decoded payload is too short to even contain a
        // version header — this returns MalformedHeader before bitcode ever runs.
        link.handle_receive(&[0x01, 0x00]);
        assert!(matches!(link.poll_receive(), Err(Error::MalformedHeader)));
    }

    // --- Version header ---

    #[test]
    fn test_version_wire_roundtrip() {
        for v in [
            Version {
                major: 0,
                minor: 0,
                patch: 0,
            },
            Version {
                major: 1,
                minor: 2,
                patch: 3,
            },
            Version {
                major: 65535,
                minor: 65535,
                patch: 65535,
            },
        ] {
            assert_eq!(Version::from_wire_bytes(v.to_wire_bytes()), v);
        }
    }

    #[test]
    fn test_version_is_compatible_with() {
        let v = |major, minor, patch| Version {
            major,
            minor,
            patch,
        };
        let cases = [
            (v(1, 2, 3), v(1, 9, 9), true),
            (v(1, 2, 3), v(2, 0, 0), false),
            (v(0, 3, 1), v(0, 3, 9), true),
            (v(0, 3, 1), v(0, 4, 0), false),
            (v(0, 0, 1), v(0, 0, 2), false),
            (v(0, 0, 1), v(0, 0, 1), true),
        ];
        for (a, b, expected) in cases {
            assert_eq!(
                a.is_compatible_with(b),
                expected,
                "{a} vs {b} expected compatible={expected}"
            );
        }
    }

    /// Hand-encodes a frame with an arbitrary (possibly non-LOCAL) version header,
    /// bypassing `to_slice`'s use of `Version::LOCAL`, so mismatch behavior can be
    /// tested directly.
    fn encode_with_version(version: Version, message: &host_to_mote::Message) -> Vec<u8> {
        let body = bitcode::serialize(message).unwrap();
        let mut plain = Vec::with_capacity(VERSION_HEADER_LEN + body.len());
        plain.extend_from_slice(&version.to_wire_bytes());
        plain.extend_from_slice(&body);
        let encoded_size = corncobs::max_encoded_len(plain.len());
        let mut cobs_buf = vec![0u8; encoded_size];
        let n = corncobs::encode_buf(&plain, &mut cobs_buf);
        cobs_buf.truncate(n);
        cobs_buf
    }

    #[test]
    fn test_version_mismatch_detected() {
        let mismatched = Version {
            major: Version::LOCAL.major,
            minor: Version::LOCAL.minor.wrapping_add(1),
            patch: Version::LOCAL.patch,
        };
        let mut link = HostConfigLink::new();
        link.handle_receive(&encode_with_version(
            mismatched,
            &host_to_mote::Message::Ping,
        ));

        match link.poll_receive() {
            Err(Error::VersionMismatch {
                local,
                remote,
                local_role,
                remote_role,
                ..
            }) => {
                assert_eq!(local, Version::LOCAL);
                assert_eq!(remote, mismatched);
                assert_eq!(local_role, Role::Mote);
                assert_eq!(remote_role, Role::Host);
            }
            other => panic!("expected VersionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_version_mismatch_does_not_strand_a_good_frame_behind_it() -> Result<(), Error> {
        let mismatched = Version {
            major: Version::LOCAL.major,
            minor: Version::LOCAL.minor.wrapping_add(1),
            patch: Version::LOCAL.patch,
        };
        let mut link = HostConfigLink::new();
        link.handle_receive(&encode_with_version(
            mismatched,
            &host_to_mote::Message::Ping,
        ));

        // A normally-encoded (LOCAL-versioned) good frame appended right behind it.
        // MoteConfigLink is used here purely as an encoder for a host_to_mote::Message
        // (the type HostConfigLink decodes) — it plays the role of "host" sending.
        let mut good = MoteConfigLink::new();
        good.send(host_to_mote::Message::Pong)?;
        while let Some(payload) = good.poll_transmit() {
            link.handle_receive(&payload);
        }

        assert!(matches!(
            link.poll_receive(),
            Err(Error::VersionMismatch { .. })
        ));
        assert_eq!(link.poll_receive()?, Some(host_to_mote::Message::Pong));
        Ok(())
    }

    #[test]
    fn test_message_role_matches_link_direction() {
        assert_eq!(<mote_to_host::Message as MessageRole>::RECEIVER, Role::Host);
        assert_eq!(<host_to_mote::Message as MessageRole>::RECEIVER, Role::Mote);
    }

    // --- Receive buffer is capped at MAX_MESSAGE_LENGTH ---

    #[test]
    fn test_receive_buffer_overflow() -> Result<(), Error> {
        let mut link = MoteLink::new();
        // Feed more bytes than MAX_MESSAGE_LENGTH with no terminator.
        let data = vec![0xABu8; MAX_MESSAGE_LENGTH + 500];
        link.handle_receive(&data);
        // No zero byte in the buffer so poll_receive returns None, not an error.
        assert!(link.poll_receive()?.is_none());
        Ok(())
    }
}
