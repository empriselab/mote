//! Foreign function interface for C++ (via the `cxx` crate)

use mote_api::MoteLink as MoteApiLink;
use mote_api::messages::{host_to_mote, mote_to_host};

use crate::MoteCommsFFI;

type MoteLinkFFI = MoteCommsFFI<1400, mote_to_host::Message, host_to_mote::Message>;

#[cxx::bridge(namespace = "mote")]
mod ffi {
    /// None on success, otherwise an error identifier
    #[derive(Debug)]
    enum MoteLinkErrorCode {
        /// No error.
        None,
        /// The underlying mote-api link/comms layer reported an error (e.g. a
        /// decode failure or version mismatch).
        Protocol,
        /// A message failed to serialize to or deserialize from JSON.
        Serialization,
    }

    /// Result of `MoteLink::send`.
    #[derive(Debug)]
    struct SendResult {
        error: MoteLinkErrorCode,
        /// Empty unless `error != None`.
        error_message: String,
    }

    /// Result of `MoteLink::poll_receive`.
    #[derive(Debug)]
    struct ReceiveResult {
        error: MoteLinkErrorCode,
        /// Empty unless `error != None`.
        error_message: String,
        /// The next decoded mote-to-host message, JSON-encoded.
        json_message: String,
    }

    extern "Rust" {
        type MoteLink;

        /// Creates a new, unconnected link.
        fn new_mote_link() -> Box<MoteLink>;

        /// Queues a JSON-encoded host-to-mote message for transmission.
        fn send(self: &mut MoteLink, json_message: &str) -> SendResult;

        /// Returns the next outbound packet's bytes, or an empty vector if
        /// nothing is queued.
        fn poll_transmit(self: &mut MoteLink) -> Vec<u8>;

        /// Feeds a received packet into the link.
        fn handle_receive(self: &mut MoteLink, packet: &[u8]);

        /// Returns the next decoded inbound message, or a result with an
        /// empty `json_message` if nothing is ready yet.
        fn poll_receive(self: &mut MoteLink) -> ReceiveResult;
    }
}

struct MoteLink {
    inner: MoteLinkFFI,
}

fn new_mote_link() -> Box<MoteLink> {
    Box::new(MoteLink {
        inner: MoteCommsFFI::from(MoteApiLink::new()),
    })
}

fn classify_error(e: &crate::Error) -> ffi::MoteLinkErrorCode {
    match e {
        crate::Error::MoteCommsError(_) => ffi::MoteLinkErrorCode::Protocol,
        crate::Error::SerdeJson(_) => ffi::MoteLinkErrorCode::Serialization,
    }
}

impl MoteLink {
    fn send(&mut self, json_message: &str) -> ffi::SendResult {
        match self.inner.send(json_message) {
            Ok(()) => ffi::SendResult {
                error: ffi::MoteLinkErrorCode::None,
                error_message: String::new(),
            },
            Err(e) => ffi::SendResult {
                error: classify_error(&e),
                error_message: e.to_string(),
            },
        }
    }

    fn poll_transmit(&mut self) -> Vec<u8> {
        self.inner.link.poll_transmit().unwrap_or_default()
    }

    fn handle_receive(&mut self, packet: &[u8]) {
        self.inner.link.handle_receive(packet);
    }

    fn poll_receive(&mut self) -> ffi::ReceiveResult {
        match self.inner.poll_receive() {
            Ok(Some(json)) => ffi::ReceiveResult {
                error: ffi::MoteLinkErrorCode::None,
                error_message: String::new(),
                json_message: json,
            },
            Ok(None) => ffi::ReceiveResult {
                error: ffi::MoteLinkErrorCode::None,
                error_message: String::new(),
                json_message: String::new(),
            },
            Err(e) => ffi::ReceiveResult {
                error: classify_error(&e),
                error_message: e.to_string(),
                json_message: String::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_api::{HostLink, messages::mote_to_host};

    fn send_ping(link: &mut MoteLink) {
        let result = link.send("\"Ping\"");
        assert_eq!(result.error, ffi::MoteLinkErrorCode::None);
    }

    #[test]
    fn test_cxx_ffi_new() {
        let _link = new_mote_link();
    }

    #[test]
    fn test_cxx_ffi_send_and_poll_transmit() {
        let mut link = new_mote_link();
        send_ping(&mut link);

        let packet = link.poll_transmit();
        assert!(!packet.is_empty());

        let packet2 = link.poll_transmit();
        assert!(packet2.is_empty());
    }

    #[test]
    fn test_cxx_ffi_send_invalid_json() {
        let mut link = new_mote_link();
        let result = link.send("not valid json");
        assert_eq!(result.error, ffi::MoteLinkErrorCode::Serialization);
        assert!(!result.error_message.is_empty());
    }

    #[test]
    fn test_cxx_ffi_poll_receive_decode_error() {
        let mut link = new_mote_link();
        // A lone non-zero, non-terminated byte followed by a terminator is not a
        // valid COBS frame long enough to contain a version header.
        let bad_frame = [0x01u8, 0x00];
        link.handle_receive(&bad_frame);

        let result = link.poll_receive();
        assert_eq!(result.error, ffi::MoteLinkErrorCode::Protocol);
        assert!(!result.error_message.is_empty());
        assert!(result.json_message.is_empty());
    }

    #[test]
    fn test_cxx_ffi_poll_receive_empty() {
        let mut link = new_mote_link();
        let result = link.poll_receive();
        assert_eq!(result.error, ffi::MoteLinkErrorCode::None);
        assert!(result.json_message.is_empty());
    }

    #[test]
    fn test_cxx_ffi_round_trip() {
        let mut link = new_mote_link();
        send_ping(&mut link);

        let packet = link.poll_transmit();
        assert!(!packet.is_empty());

        let mut mote = HostLink::new();
        mote.handle_receive(&packet);
        let received = mote.poll_receive().unwrap().unwrap();
        assert_eq!(received, host_to_mote::Message::Ping);
    }

    #[test]
    fn test_cxx_ffi_handle_receive_and_poll_receive() {
        let mut mote = HostLink::new();
        mote.send(mote_to_host::Message::Pong).unwrap();
        let payload = mote.poll_transmit().unwrap();

        let mut link = new_mote_link();
        link.handle_receive(&payload);

        let result = link.poll_receive();
        assert_eq!(result.error, ffi::MoteLinkErrorCode::None);
        assert!(!result.json_message.is_empty());

        let msg: mote_to_host::Message = serde_json::from_str(&result.json_message).unwrap();
        assert_eq!(msg, mote_to_host::Message::Pong);
    }
}
