//! The raw dispatcher does not interpret the data in the packet.
//!
//! It can be used as a debug dispatcher when a dispatcher does not exist
//! for a particular packet.
use std::convert::From;

use super::Dispatcher;

/// Implements the `Dispatcher` trait for unknown dispatch schemes.
///
/// TODO(Kaarel)
pub struct RawDispatcher {
    #[allow(dead_code)]
    dispatch: u8,
}

impl RawDispatcher {
    /// Creates a new instance of the `RawDispatcher`.
    ///
    /// The `dispatch` byte is used to select the dispatcher scheme.
    pub fn new(dispatch: u8) -> Self {
        RawDispatcher { dispatch }
    }
}

impl Dispatcher for RawDispatcher {}

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
