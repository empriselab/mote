//! Foreign function interfaces for TypeScript (WASM)

use wasm_bindgen::prelude::*;

use gloo_utils::format::JsValueSerdeExt;

use crate::Error;
use mote_api::MoteConfigLink;
use mote_api::messages::{host_to_mote, mote_to_host};

// Let wasm-bindgen throw a real JS `Error` object (so `catch (e) { e.message }`
// works on the JS/TS side) rather than a bare string.
impl From<Error> for JsValue {
    fn from(err: Error) -> JsValue {
        js_sys::Error::new(&err.to_string()).into()
    }
}

/// WASM/TS binding for the mote-configuration web UI's wifi-setup link.
///
/// This wraps [`MoteConfigLink`] (the serial/USB-MTU link), not [`MoteLink`]
/// (mote-api's UDP-MTU link, used by the Python and C bindings) — the
/// configuration UI talks to Mote over the same USB-serial connection used to
/// initially set up wifi (see mote-book's "Configuration" page), not over the
/// network, so it needs the small-MTU framing.
///
/// [`MoteLink`]: mote_api::MoteLink
#[wasm_bindgen]
pub struct Link {
    link: MoteConfigLink,
}

#[wasm_bindgen]
impl Link {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            link: MoteConfigLink::new(),
        }
    }

    pub fn send(&mut self, msg: JsValue) -> Result<(), Error> {
        let message: host_to_mote::Message = JsValue::into_serde(&msg)?;
        self.link.send(message)?;
        Ok(())
    }

    pub fn poll_transmit(&mut self) -> Result<JsValue, Error> {
        if let Some(payload) = self.link.poll_transmit() {
            Ok(JsValue::from_serde(&payload)?)
        } else {
            Ok(JsValue::from_serde(&())?)
        }
    }

    pub fn handle_receive(&mut self, bytes: JsValue) -> Result<(), Error> {
        let bytes: Vec<u8> = JsValue::into_serde(&bytes)?;
        self.link.handle_receive(&bytes);
        Ok(())
    }

    pub fn poll_receive(&mut self) -> Result<JsValue, Error> {
        let message: Option<mote_to_host::Message> = self.link.poll_receive()?;
        Ok(JsValue::from_serde(&message)?)
    }
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

// wasm-bindgen's JsValue conversions call into imported JS functions and only
// work under an actual JS host, so this module only builds for wasm32 and only
// runs for real via `wasm-pack test --node --features wasm_ffi` (wired into
// `task ffi:test-wasm`) -- not plain `cargo test`, which targets the host arch.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use mote_api::HostLink;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn test_send_and_poll_transmit() {
        let mut link = Link::new();
        let msg = JsValue::from_serde(&host_to_mote::Message::Ping).unwrap();
        link.send(msg).unwrap();

        let packet = link.poll_transmit().unwrap();
        assert!(!packet.is_undefined());
        let bytes: Vec<u8> = JsValue::into_serde(&packet).unwrap();
        assert!(!bytes.is_empty());

        // Nothing left to transmit: `poll_transmit` reports this as JS `null`
        // (from serializing `()`), not an empty array.
        let empty = link.poll_transmit().unwrap();
        assert!(empty.is_null());
    }

    #[wasm_bindgen_test]
    fn test_round_trip_with_host_link() {
        let mut link = Link::new();
        let msg = JsValue::from_serde(&host_to_mote::Message::Ping).unwrap();
        link.send(msg).unwrap();

        let packet = link.poll_transmit().unwrap();
        let bytes: Vec<u8> = JsValue::into_serde(&packet).unwrap();

        let mut host = HostLink::new();
        host.handle_receive(&bytes);
        assert_eq!(
            host.poll_receive().unwrap().unwrap(),
            host_to_mote::Message::Ping
        );
    }

    #[wasm_bindgen_test]
    fn test_handle_receive_and_poll_receive() {
        let mut mote = HostLink::new();
        mote.send(mote_to_host::Message::Pong).unwrap();
        let payload = mote.poll_transmit().unwrap();

        let mut link = Link::new();
        let bytes_js = JsValue::from_serde(&payload).unwrap();
        link.handle_receive(bytes_js).unwrap();

        let received = link.poll_receive().unwrap();
        let message: Option<mote_to_host::Message> = JsValue::into_serde(&received).unwrap();
        assert_eq!(message, Some(mote_to_host::Message::Pong));
    }

    #[wasm_bindgen_test]
    fn test_send_invalid_message_is_an_error() {
        let mut link = Link::new();
        let bad = JsValue::from_str("not a valid message");
        assert!(link.send(bad).is_err());
    }
}
