//! Transport layer implementations enable receiving and sending packets
//! using different communication protocols.
//!
//! A transport layer is responsible the low level communication between
//! the radio module to the `moteconnection` library.
use std::sync::mpsc::{Receiver, Sender};

use crate::Bytes;

pub mod serial;
pub mod serialforwarder;

/// Lists events that can be sent to and from the Transport implementation.
#[derive(Debug)]
pub enum Event {
    /// The connection has been established.
    Connected,
    /// The connection has been closed.
    Disconnected,
    /// Contains the data to be sent or has been received.
    Data(Bytes),
    /// Signals the shutdown of the transport.
    Stop,
}

/// Provides a handle for the use in `Connection`.
pub struct TransportHandle {
    /// The sender for the outging channel
    pub tx: Sender<Event>,
    /// the receiver for the incoming channel
    pub rx: Receiver<Event>,
}

/// An object responsible for the transportation of moteconnection packets.
pub trait Transport {
    /// Stops an already open connection.
    fn stop(self) -> Result<(), &'static str>;

    /// Returns the handle for the transport.
    fn get_handle(&mut self) -> TransportHandle;
}
