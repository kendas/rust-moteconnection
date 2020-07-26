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

/// A struct representing a transport implementation.
pub struct Transport {
    handle: Option<TransportHandle>,
    /// The stopper function for the `Trasnport`
    stopper: Box<dyn FnOnce() -> Result<(), &'static str>>,
}

/// Provides a handle for the use in `Connection`.
pub struct TransportHandle {
    /// The sender for the outging channel
    pub tx: Sender<Event>,
    /// the receiver for the incoming channel
    pub rx: Receiver<Event>,
}

/// Provides the ability to create a `Transport` instance.
pub trait TransportBuilder {
    /// Starts the transport manager and returns the handle.
    fn start(&self) -> Transport;
}

impl Transport {
    /// Creates a new Transport.
    pub fn new(tx: Sender<Event>, rx: Receiver<Event>) -> Transport {
        Transport::with_stopper(tx, rx, Box::new(|| Ok(())))
    }

    /// Creates a new `Transport` with a function to be called when stopping.
    pub fn with_stopper(
        tx: Sender<Event>,
        rx: Receiver<Event>,
        stopper: Box<dyn FnOnce() -> Result<(), &'static str>>,
    ) -> Transport {
        Transport {
            handle: Some(TransportHandle { tx, rx }),
            stopper,
        }
    }

    /// Returns the `TransportHandle`
    ///
    /// TODO(Kaarel): panic warning etc.
    pub fn get_handle(&mut self) -> TransportHandle {
        self.handle.take().unwrap()
    }

    /// Stops the transport.
    pub fn stop(self) -> Result<(), &'static str> {
        (self.stopper)()
    }
}
