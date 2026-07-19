//! Foreign function interface for Python

#[pyo3::pymodule]
mod mote_ffi {
    use pyo3::{exceptions::PyValueError, prelude::*};
    use std::string::{String, ToString};

    use crate::{Error, MoteCommsFFI};
    use mote_api::{
        MoteLink,
        messages::{host_to_mote, mote_to_host},
    };

    impl std::convert::From<Error> for PyErr {
        fn from(err: Error) -> PyErr {
            PyValueError::new_err(err.to_string())
        }
    }

    /// The Rust <-> JSON link underlying `mote_link.link.MoteClient`.
    ///
    /// Every method takes or returns JSON strings matching mote-api's wire
    /// format (see the schemas in mote-ffi/schemas/).
    #[pyclass]
    struct Link {
        link: MoteCommsFFI<1400, mote_to_host::Message, host_to_mote::Message>,
    }

    #[pymethods]
    impl Link {
        /// Creates a new, unconnected link.
        #[new]
        fn new() -> Self {
            Self {
                link: MoteCommsFFI::from(MoteLink::new()),
            }
        }

        /// Queues `message` (a JSON-encoded host-to-mote message) to be sent.
        fn send(&mut self, message: String) -> Result<(), Error> {
            self.link.send(&message)?;
            Ok(())
        }

        /// Returns the next raw packet to transmit, JSON-encoded as an array
        /// of byte values, or `None` if nothing is queued.
        fn poll_transmit(&mut self) -> Result<Option<String>, Error> {
            self.link.poll_transmit()
        }

        /// Feeds a raw received packet (JSON-encoded as an array of byte
        /// values) into the link.
        fn handle_receive(&mut self, packet: String) -> Result<(), Error> {
            self.link.handle_receive(&packet)
        }

        /// Returns the next decoded message (a JSON-encoded mote-to-host
        /// message), or `None` if no complete message is ready yet.
        fn poll_receive(&mut self) -> Result<Option<String>, Error> {
            self.link.poll_receive()
        }
    }
}
