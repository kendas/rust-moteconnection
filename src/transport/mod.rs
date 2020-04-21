//! Transport layer implementations enable receiving and sending packets
//! using different communication protocols.
//!
//! A transport layer is responsible the low level communication between
//! the radio module to the `moteconnection` library.
use std::sync::mpsc::TryRecvError;

pub mod serial;
pub mod serialforwarder;

/// An object responsible for the transportation of moteconnection packets.
pub trait Transport {
    /// Stops an already open connection.
    fn stop(self) -> Result<(), &'static str>;
    /// Sends a message to the connected device.
    fn send_packet(&self, data: Vec<u8>) -> Result<(), &'static str>;
    /// Receives a message from the connected device.
    fn receive_packet(&self) -> Result<Vec<u8>, TryRecvError>;
}
