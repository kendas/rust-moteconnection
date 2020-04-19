//! The raw dispatcher does not interpret the data in the packet.
//!
//! It can be used as a debug dispatcher when a dispatcher does not exist
//! for a particular packet.
use super::Dispatcher;

/// Implements the `Dispatcher` trait for unknown dispatch schemes.
///
/// TODO(Kaarel)
pub struct RawDispatcher {
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
