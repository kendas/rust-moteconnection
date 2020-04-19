//! The ActiveMessage dispatch scheme is used for sending addressed packets.
//!
//! The ActiveMessage dispatch scheme has the dispatch byte of 0x00.
//! The general structure is the following:
//!
//! ```text
//! ┌──┬──┬─┬─┬─┬──────────╱┄╱────────┐
//! │  │  │ │ │ │         ╱ ╱         │
//! └──┴──┴─┴─┴─┴────────╱┄╱──────────┘
//!  DD SS L G A       Payload
//! ```
//!
//! Where the fields are:
//!
//! | Symbol  | Length   | Description              | Range         |
//! |---------|----------|--------------------------|---------------|
//! | DD      | 2 bytes  | Destination (Big-endian) | 0x0000-0xFFFF |
//! | SS      | 2 bytes  | Source (Big-endian)      | 0x0000-0xFFFF |
//! | L       | 1 byte   | Payload length           | 0x00-0x??     |
//! | G       | 1 byte   | AM group                 | 0x00-0xFF     |
//! | A       | 1 byte   | AM ID                    | 0x00-0xFF     |
//! | Payload | variable | Data payload             |               |
//!
//! ## More information
//!
//! There is some more information in the [Serial protocol][1] documentation.
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/Serial-protocol
use super::Dispatcher;

/// Implements the `Dispatcher` trait for the ActiveMessage dispatch scheme.
///
/// TODO(Kaarel)
pub struct AMDispatcher {
    dispatch: u8,
}

impl AMDispatcher {
    /// Creates a new `AMDispatcher`
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for AMDispatcher {
    fn default() -> Self {
        AMDispatcher { dispatch: 0x00 }
    }
}

impl Dispatcher for AMDispatcher {}

/// TODO(Kaarel)
pub struct Message {
    /// Destination
    pub dest: u16,
    /// Source
    pub src: u16,
    /// The length of the payload
    pub length: u8,
    /// The AM group
    pub group: u8,
    /// The AM ID
    pub id: u8,
    /// The payload
    pub payload: Vec<u8>,
}
