//! The raw dispatcher does not interpret the data in the packet.
//!
//! It can be used as a debug dispatcher when a dispatcher does not exist
//! for a particular packet.
use std::convert::From;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use super::{Bytes, Dispatcher, DispatcherBuilder, DispatcherHandle};

/// Handles the dispatching of raw packets without interpreting the contents.
pub struct RawDispatcher {
    dispatch: u8,
    /// The receiver
    pub rx: Receiver<Bytes>,
    /// The sender
    pub tx: Sender<Bytes>,
    transport_rx: Receiver<Bytes>,
    transport_tx: Sender<Bytes>,
}

impl RawDispatcher {
    /// Creates a raw packet dispatcher
    pub fn new(dispatch: u8) -> RawDispatcher {
        let (transport_tx, rx) = mpsc::channel();
        let (tx, transport_rx) = mpsc::channel();
        RawDispatcher {
            dispatch,
            rx,
            tx,
            transport_rx,
            transport_tx,
        }
    }
}

impl DispatcherBuilder for RawDispatcher {
    fn create(self) -> Dispatcher {
        Dispatcher {
            dispatch_byte: self.dispatch,
            handle: Arc::new(Mutex::new(DispatcherHandle {
                rx: self.transport_rx,
                tx: self.transport_tx,
            })),
            rx: self.rx,
            tx: self.tx,
        }
    }
}

/// A raw packet payload
pub struct Packet {
    /// The payload of the packet
    pub payload: Vec<u8>,
}

// TODO(Kaarel): Technically, there are limits to payload sizes
impl From<Vec<u8>> for Packet {
    fn from(bytes: Vec<u8>) -> Self {
        Packet { payload: bytes }
    }
}
