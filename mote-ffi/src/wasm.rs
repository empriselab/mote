//! Foreign function interfaces for TypeScript (WASM)

use wasm_bindgen::prelude::*;

use gloo_utils::format::JsValueSerdeExt;

use crate::Error;
use mote_api::MoteConfigLink;
use mote_api::messages::{host_to_mote, mote_to_host};

// Let wasm-bindgen throw a JS exception carrying the error message when a
// `Link` method returns `Err`.
impl From<Error> for JsValue {
    fn from(err: Error) -> JsValue {
        JsValue::from_str(&err.to_string())
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[allow(dead_code)]
#[wasm_bindgen]
struct Link {
    link: MoteConfigLink,
}
#[allow(dead_code)]
#[wasm_bindgen]
impl Link {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            link: MoteConfigLink::new(),
        }
    }

    pub fn send(&mut self, msg: JsValue) -> Result<(), Error> {
        console_log!("[TX] Configuration link send: {:?}", msg);
        let message: host_to_mote::Message = JsValue::into_serde(&msg)?;
        console_log!("[TX] Configuration link unpacked: {:?}", message);
        self.link.send(message)?;
        console_log!("[TX] Message queued for send");
        Ok(())
    }

    pub fn poll_transmit(&mut self) -> Result<JsValue, Error> {
        if let Some(payload) = self.link.poll_transmit() {
            console_log!("[TX] Sending {:?}", payload);
            Ok(JsValue::from_serde(&payload)?)
        } else {
            Ok(JsValue::from_serde(&())?)
        }
    }

    pub fn handle_receive(&mut self, bytes: JsValue) -> Result<(), Error> {
        let bytes: Vec<u8> = JsValue::into_serde(&bytes)?;
        console_log!("[RX] Configuration link received: {:?}", bytes);
        self.link.handle_receive(&bytes);
        Ok(())
    }

    pub fn poll_receive(&mut self) -> Result<JsValue, Error> {
        let message: Option<mote_to_host::Message> = self.link.poll_receive()?;
        console_log!("[RX] Configuration link unpacked: {:?}", message);
        Ok(JsValue::from_serde(&message)?)
    }
}
