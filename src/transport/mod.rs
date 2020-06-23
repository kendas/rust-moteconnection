//! Transport layer implementations enable receiving and sending packets
//! using different communication protocols.
//!
//! A transport layer is responsible the low level communication between
//! the radio module to the `moteconnection` library.
use std::sync::mpsc::{Receiver, Sender, TryRecvError, TrySendError};
use std::thread::JoinHandle;

pub mod serial;
pub mod serialforwarder;

type Bytes = Vec<u8>;

/// Manages the transportation of moteconnection packets over the TCP
/// serial-formarder protocol.
pub struct Transport {
    tx: Sender<Bytes>,
    rx: Receiver<Bytes>,
    halt: Sender<()>,
    handle: JoinHandle<()>,
}

/// An object responsible for the transportation of moteconnection packets.
impl Transport {
    /// Sends a message to the connected device.
    fn send_packet(&self, data: Bytes) -> Result<(), TrySendError<Bytes>> {
        self.tx.send(data)?;
        Ok(())
    }
    /// Receives a message from the connected device.
    fn receive_packet(&self) -> Result<Bytes, TryRecvError> {
        Ok(self.rx.try_recv()?)
    }
    /// Stops an already open connection.
    pub fn stop(self) -> Result<(), &'static str> {
        if self.halt.send(()).is_err() {
            return Err("Control receiver closed!");
        };
        if self.handle.join().is_err() {
            return Err("Unable to join thread!");
        }
        Ok(())
    }
}
