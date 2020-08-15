//! The raw dispatcher does not interpret the data in the packet.
//!
//! It can be used as a debug dispatcher when a dispatcher does not exist
//! for a particular packet.
use std::convert::From;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use crate::{Bytes, Event};
use super::{Dispatcher, DispatcherHandle};

/// Handles the dispatching of raw packets without interpreting the contents.
pub struct RawDispatcher {
    dispatch: u8,
    /// The receiver
    pub rx: Receiver<Event<Bytes>>,
    /// The sender
    pub tx: Sender<Event<Bytes>>,
    handle: Option<DispatcherHandle>,
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
            handle: Some(DispatcherHandle::new(transport_tx, transport_rx)),
        }
    }

    /// Returns the receiver for data from the serial device.
    pub fn rx(&self) -> &Receiver<Event<Bytes>> {
        &self.rx
    }

    /// Returns the sender for data to the serial device.
    pub fn tx(&self) -> &Sender<Event<Bytes>> {
        &self.tx
    }
}

impl Dispatcher for RawDispatcher {
    fn dispatch_byte(&self) -> u8 {
        self.dispatch
    }

    fn get_handle(&mut self) -> DispatcherHandle {
        self.handle.take().unwrap()
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
