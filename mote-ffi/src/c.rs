//! Foreign function interface for C

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use mote_api::{
    MoteLink,
    messages::{host_to_mote, mote_to_host},
};
use serde::Serialize;

use crate::MoteCommsFFI;

type MoteLinkFFI = MoteCommsFFI<1400, mote_to_host::Message, host_to_mote::Message>;

pub struct MoteLinkHandle {
    inner: MoteLinkFFI,
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    error: &'a str,
}

/// Coarse, programmatically dispatchable error classification for the C ABI.
///
/// `None` (0) means the call succeeded. Every other variant means the call
/// failed; a human-readable message is additionally written to the caller's
/// `buf` as `{"error": "<message>"}` where the function signature allows for it.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoteLinkErrorCode {
    /// No error; the call succeeded.
    None = 0,
    /// `buf_len` was negative.
    InvalidBufLen,
    /// A provided string argument was not valid UTF-8.
    InvalidUtf8,
    /// `buf` was too small to hold the result (or the error message describing
    /// another failure).
    BufferTooSmall,
    /// The underlying mote-api link/comms layer reported an error (e.g. a
    /// decode failure or version mismatch).
    Protocol,
    /// A message failed to serialize to or deserialize from JSON, or could
    /// not be represented as a null-terminated C string.
    Serialization,
}

/// Result of a C ABI call that may produce data.
///
/// `bytes_written` and `error` are both meaningful: on success (`error ==
/// MoteLinkErrorCode::None`) `bytes_written` reports how much data was
/// written to `buf` (0 if none was ready). On failure, `bytes_written`
/// reports how many bytes of the JSON-encoded error message were written to
/// `buf` instead, or -1 if not even that fit.
#[repr(C)]
pub struct MoteLinkResult {
    pub bytes_written: c_int,
    pub error: MoteLinkErrorCode,
}

/// Converts a caller-provided buffer length to `usize`, rejecting negative values.
fn checked_buf_len(buf_len: c_int) -> Option<usize> {
    usize::try_from(buf_len).ok()
}

/// Write a JSON-encoded error object (`{"error": "<message>"}`) as a
/// null-terminated string into `buf`.
///
/// On success, returns the number of bytes written including the null terminator,
/// On error, returns -1
///
/// # Safety
/// `buf` must point to a writable buffer of at least `buf_len` bytes.
unsafe fn write_error_json(buf: *mut c_char, buf_len: c_int, message: &str) -> c_int {
    let Some(buf_len) = checked_buf_len(buf_len) else {
        return -1;
    };
    let json = match serde_json::to_string(&ErrorPayload { error: message }) {
        Ok(j) => j,
        Err(_) => return -1,
    };
    let cstr = match CString::new(json) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let bytes = cstr.as_bytes_with_nul();
    if bytes.len() > buf_len {
        return -1;
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, bytes.len()) };
    bytes.len() as c_int
}

/// Classifies a `mote_ffi::Error` into the coarse `MoteLinkErrorCode` reported
/// across the C ABI.
fn classify_error(e: &crate::Error) -> MoteLinkErrorCode {
    match e {
        crate::Error::MoteCommsError(_) => MoteLinkErrorCode::Protocol,
        crate::Error::SerdeJson(_) => MoteLinkErrorCode::Serialization,
    }
}

/// Writes a JSON-encoded error message to `buf` and builds the `MoteLinkResult`
/// describing the outcome.
///
/// If `buf_len` is negative or `buf` is too small to hold the message, the
/// result's error code reports that instead of `fallback_error` — the
/// original error's details are only available via `buf` when it fits.
///
/// # Safety
/// `buf` must point to a writable buffer of at least `buf_len` bytes.
unsafe fn error_result(
    buf: *mut c_char,
    buf_len: c_int,
    message: &str,
    fallback_error: MoteLinkErrorCode,
) -> MoteLinkResult {
    if checked_buf_len(buf_len).is_none() {
        return MoteLinkResult {
            bytes_written: -1,
            error: MoteLinkErrorCode::InvalidBufLen,
        };
    }
    let bytes_written = unsafe { write_error_json(buf, buf_len, message) };
    let error = if bytes_written < 0 {
        MoteLinkErrorCode::BufferTooSmall
    } else {
        fallback_error
    };
    MoteLinkResult {
        bytes_written,
        error,
    }
}

/// Create a new MoteLink handle. The returned pointer must be freed with `mote_link_free`.
#[unsafe(no_mangle)]
pub extern "C" fn mote_link_new() -> *mut MoteLinkHandle {
    Box::into_raw(Box::new(MoteLinkHandle {
        inner: MoteCommsFFI::from(MoteLink::new()),
    }))
}

/// Free a MoteLink handle.
///
/// # Safety
/// `handle` must be a valid pointer returned by `mote_link_new` and must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mote_link_free(handle: *mut MoteLinkHandle) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}

/// Queue a JSON-encoded host-to-mote message for transmission.
///
/// On success, returns `{ bytes_written: 0, error: MoteLinkErrorCode::None }`.
/// On failure, writes a JSON-encoded error object (`{"error": "<message>"}`)
/// as a null-terminated string into `buf`; `bytes_written` reports how many
/// bytes of that message were written (or -1 if not even the error could be
/// written, e.g. `buf` is too small), and `error` reports why the call failed.
///
/// # Safety
/// `handle` must be a valid non-null pointer. `json_message` must be a valid null-terminated UTF-8 string.
/// `buf` must point to a writable buffer of at least `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mote_link_send(
    handle: *mut MoteLinkHandle,
    json_message: *const c_char,
    buf: *mut c_char,
    buf_len: c_int,
) -> MoteLinkResult {
    let handle = unsafe { &mut *handle };
    let json = match unsafe { CStr::from_ptr(json_message) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            return unsafe {
                error_result(
                    buf,
                    buf_len,
                    "invalid UTF-8 in json_message",
                    MoteLinkErrorCode::InvalidUtf8,
                )
            };
        }
    };
    match handle.inner.send(json) {
        Ok(_) => MoteLinkResult {
            bytes_written: 0,
            error: MoteLinkErrorCode::None,
        },
        Err(e) => {
            let code = classify_error(&e);
            unsafe { error_result(buf, buf_len, &e.to_string(), code) }
        }
    }
}

/// Copy the next transmit packet into `buf`.
///
/// On success, `bytes_written` is the number of bytes written (0 if there is
/// no packet to transmit) and `error` is `MoteLinkErrorCode::None`. On
/// failure (`buf` too small, or `buf_len` negative), `bytes_written` is -1
/// and `error` reports why.
///
/// # Safety
/// `handle` must be a valid non-null pointer. `buf` must point to a writable buffer of at least `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mote_link_poll_transmit(
    handle: *mut MoteLinkHandle,
    buf: *mut u8,
    buf_len: c_int,
) -> MoteLinkResult {
    let Some(buf_len) = checked_buf_len(buf_len) else {
        return MoteLinkResult {
            bytes_written: -1,
            error: MoteLinkErrorCode::InvalidBufLen,
        };
    };
    let handle = unsafe { &mut *handle };
    match handle.inner.link.poll_transmit() {
        Some(bytes) => {
            if bytes.len() > buf_len {
                return MoteLinkResult {
                    bytes_written: -1,
                    error: MoteLinkErrorCode::BufferTooSmall,
                };
            }
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len()) };
            MoteLinkResult {
                bytes_written: bytes.len() as c_int,
                error: MoteLinkErrorCode::None,
            }
        }
        None => MoteLinkResult {
            bytes_written: 0,
            error: MoteLinkErrorCode::None,
        },
    }
}

/// Feed a received packet into the link.
///
/// Returns `MoteLinkErrorCode::None` on success, or
/// `MoteLinkErrorCode::InvalidBufLen` if `buf_len` is negative.
///
/// # Safety
/// `handle` must be a valid non-null pointer. `buf` must point to a readable buffer of at least `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mote_link_handle_receive(
    handle: *mut MoteLinkHandle,
    buf: *const u8,
    buf_len: c_int,
) -> MoteLinkErrorCode {
    let Some(buf_len) = checked_buf_len(buf_len) else {
        return MoteLinkErrorCode::InvalidBufLen;
    };
    let handle = unsafe { &mut *handle };
    let bytes = unsafe { std::slice::from_raw_parts(buf, buf_len) };
    handle.inner.link.handle_receive(bytes);
    MoteLinkErrorCode::None
}

/// Copy the next decoded mote-to-host message as a null-terminated JSON string into `buf`.
///
/// On success, `bytes_written` is the number of bytes written including the
/// null terminator (0 if no message is ready) and `error` is
/// `MoteLinkErrorCode::None`. On failure, writes a JSON-encoded error object
/// (`{"error": "<message>"}`) into `buf`; `bytes_written` reports how many
/// bytes of that message were written (or -1 if not even the error could be
/// written), and `error` reports why the call failed.
///
/// # Safety
/// `handle` must be a valid non-null pointer. `buf` must point to a writable buffer of at least `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mote_link_poll_receive(
    handle: *mut MoteLinkHandle,
    buf: *mut c_char,
    buf_len: c_int,
) -> MoteLinkResult {
    let Some(checked_len) = checked_buf_len(buf_len) else {
        return MoteLinkResult {
            bytes_written: -1,
            error: MoteLinkErrorCode::InvalidBufLen,
        };
    };
    let handle = unsafe { &mut *handle };
    match handle.inner.poll_receive() {
        Ok(Some(json)) => {
            let cstr = match CString::new(json) {
                Ok(s) => s,
                Err(_) => {
                    return unsafe {
                        error_result(
                            buf,
                            buf_len,
                            "message contained an embedded null byte",
                            MoteLinkErrorCode::Serialization,
                        )
                    };
                }
            };
            let bytes = cstr.as_bytes_with_nul();
            if bytes.len() > checked_len {
                return unsafe {
                    error_result(
                        buf,
                        buf_len,
                        "message too large for buffer",
                        MoteLinkErrorCode::BufferTooSmall,
                    )
                };
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, bytes.len())
            };
            MoteLinkResult {
                bytes_written: bytes.len() as c_int,
                error: MoteLinkErrorCode::None,
            }
        }
        Ok(None) => MoteLinkResult {
            bytes_written: 0,
            error: MoteLinkErrorCode::None,
        },
        Err(e) => {
            let code = classify_error(&e);
            unsafe { error_result(buf, buf_len, &e.to_string(), code) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_api::{HostLink, messages::mote_to_host};

    unsafe fn send_ping(handle: *mut MoteLinkHandle) {
        let json = c"\"Ping\"";
        let mut buf = [0i8; 256];
        let ret =
            unsafe { mote_link_send(handle, json.as_ptr(), buf.as_mut_ptr(), buf.len() as c_int) };
        assert_eq!(ret.error, MoteLinkErrorCode::None);
    }

    /// Parses `buf` (as written by an error-returning FFI call) as `{"error": "..."}`
    /// and returns the message text, panicking if the shape doesn't match.
    unsafe fn parse_error_json(buf: &[i8]) -> String {
        let json = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("expected {{\"error\": ...}}, got {json}"))
            .to_string()
    }

    #[test]
    fn test_c_ffi_new_and_free() {
        unsafe {
            let handle = mote_link_new();
            assert!(!handle.is_null());
            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_send_and_poll_transmit() {
        unsafe {
            let handle = mote_link_new();
            send_ping(handle);

            let mut buf = [0u8; 256];
            let result = mote_link_poll_transmit(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::None);
            assert!(result.bytes_written > 0);

            let result2 = mote_link_poll_transmit(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result2.error, MoteLinkErrorCode::None);
            assert_eq!(result2.bytes_written, 0);

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_poll_transmit_buffer_too_small() {
        unsafe {
            let handle = mote_link_new();
            send_ping(handle);

            let mut buf = [0u8; 1];
            let result = mote_link_poll_transmit(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::BufferTooSmall);
            assert_eq!(result.bytes_written, -1);

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_send_invalid_json() {
        unsafe {
            let handle = mote_link_new();
            let bad = c"not valid json";
            let mut buf = [0i8; 256];
            let result = mote_link_send(handle, bad.as_ptr(), buf.as_mut_ptr(), buf.len() as c_int);
            // A recoverable failure: the error itself is reported as JSON, not a bare -1.
            assert_eq!(result.error, MoteLinkErrorCode::Serialization);
            assert!(result.bytes_written > 0);
            let message = parse_error_json(&buf);
            assert!(!message.is_empty());
            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_send_error_buffer_too_small() {
        unsafe {
            let handle = mote_link_new();
            let bad = c"not valid json";
            let mut buf = [0i8; 1];
            let result = mote_link_send(handle, bad.as_ptr(), buf.as_mut_ptr(), buf.len() as c_int);
            // Truly unrecoverable: not even the error JSON fits.
            assert_eq!(result.error, MoteLinkErrorCode::BufferTooSmall);
            assert_eq!(result.bytes_written, -1);
            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_poll_receive_decode_error_returns_json() {
        unsafe {
            let handle = mote_link_new();
            // A lone non-zero, non-terminated byte followed by a terminator is not a
            // valid COBS frame long enough to contain a version header.
            let bad_frame = [0x01u8, 0x00];
            let ret =
                mote_link_handle_receive(handle, bad_frame.as_ptr(), bad_frame.len() as c_int);
            assert_eq!(ret, MoteLinkErrorCode::None);

            let mut buf = [0i8; 256];
            let result = mote_link_poll_receive(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::Protocol);
            assert!(result.bytes_written > 0);
            let message = parse_error_json(&buf);
            assert!(!message.is_empty());

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_poll_receive_decode_error_buffer_too_small() {
        unsafe {
            let handle = mote_link_new();
            let bad_frame = [0x01u8, 0x00];
            mote_link_handle_receive(handle, bad_frame.as_ptr(), bad_frame.len() as c_int);

            let mut buf = [0i8; 1];
            let result = mote_link_poll_receive(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::BufferTooSmall);
            assert_eq!(result.bytes_written, -1);

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_poll_receive_empty() {
        unsafe {
            let handle = mote_link_new();
            let mut buf = [0i8; 256];
            let result = mote_link_poll_receive(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::None);
            assert_eq!(result.bytes_written, 0);
            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_round_trip() {
        unsafe {
            let handle = mote_link_new();
            send_ping(handle);

            let mut packet_buf = [0u8; 4096];
            let result =
                mote_link_poll_transmit(handle, packet_buf.as_mut_ptr(), packet_buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::None);
            assert!(result.bytes_written > 0);

            let mut mote = HostLink::new();
            mote.handle_receive(&packet_buf[..result.bytes_written as usize]);
            let received = mote.poll_receive().unwrap().unwrap();
            assert_eq!(received, host_to_mote::Message::Ping);

            mote_link_free(handle);
        }
    }

    // --- Negative buf_len must not bypass bounds checks ---
    //
    // Casting a negative `c_int` straight to `usize` wraps it into a huge number,
    // which would defeat every "buffer too small" check below it. Every function
    // that takes a `buf_len` must reject negative values outright.

    #[test]
    fn test_c_ffi_send_negative_buf_len() {
        // mote_link_send only touches buf/buf_len on its error path (a successful
        // send never writes into buf), so use invalid JSON to reach that path.
        unsafe {
            let handle = mote_link_new();
            let bad = c"not valid json";
            let mut buf = [0i8; 256];
            let result = mote_link_send(handle, bad.as_ptr(), buf.as_mut_ptr(), -1);
            assert_eq!(result.error, MoteLinkErrorCode::InvalidBufLen);
            assert_eq!(result.bytes_written, -1);
            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_poll_transmit_negative_buf_len() {
        unsafe {
            let handle = mote_link_new();
            send_ping(handle);

            let mut buf = [0u8; 256];
            let result = mote_link_poll_transmit(handle, buf.as_mut_ptr(), -1);
            assert_eq!(result.error, MoteLinkErrorCode::InvalidBufLen);
            assert_eq!(result.bytes_written, -1);

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_handle_receive_negative_buf_len() {
        unsafe {
            let handle = mote_link_new();
            let mut mote = HostLink::new();
            mote.send(mote_to_host::Message::Pong).unwrap();
            let payload = mote.poll_transmit().unwrap();

            let ret = mote_link_handle_receive(handle, payload.as_ptr(), -1);
            assert_eq!(ret, MoteLinkErrorCode::InvalidBufLen);

            // The (would-be out-of-bounds) read never happened, so nothing was queued.
            let mut buf = [0i8; 256];
            let result = mote_link_poll_receive(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::None);
            assert_eq!(result.bytes_written, 0);

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_poll_receive_negative_buf_len() {
        unsafe {
            let mut mote = HostLink::new();
            mote.send(mote_to_host::Message::Pong).unwrap();
            let payload = mote.poll_transmit().unwrap();

            let handle = mote_link_new();
            mote_link_handle_receive(handle, payload.as_ptr(), payload.len() as c_int);

            let mut buf = [0i8; 256];
            let result = mote_link_poll_receive(handle, buf.as_mut_ptr(), -1);
            assert_eq!(result.error, MoteLinkErrorCode::InvalidBufLen);
            assert_eq!(result.bytes_written, -1);

            mote_link_free(handle);
        }
    }

    #[test]
    fn test_c_ffi_handle_receive_and_poll_receive() {
        unsafe {
            let mut mote = HostLink::new();
            mote.send(mote_to_host::Message::Pong).unwrap();
            let payload = mote.poll_transmit().unwrap();

            let handle = mote_link_new();
            let ret = mote_link_handle_receive(handle, payload.as_ptr(), payload.len() as c_int);
            assert_eq!(ret, MoteLinkErrorCode::None);

            let mut buf = [0i8; 256];
            let result = mote_link_poll_receive(handle, buf.as_mut_ptr(), buf.len() as c_int);
            assert_eq!(result.error, MoteLinkErrorCode::None);
            assert!(result.bytes_written > 0);

            let json = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            let msg: mote_to_host::Message = serde_json::from_str(json).unwrap();
            assert_eq!(msg, mote_to_host::Message::Pong);

            mote_link_free(handle);
        }
    }
}
