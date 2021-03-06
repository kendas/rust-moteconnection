//! The ActiveMessage dispatch scheme is used for sending addressed packets.
//!
//! The ActiveMessage dispatch scheme has the dispatch byte of `0x00`.
//! The general structure is the following:
//!
//! ```text
//! ┌──┬──┬─┬─┬─┬──────────╱┄╱───────┬──────────╱┄╱────────┐
//! │  │  │ │ │ │         ╱ ╱        │         ╱ ╱         │
//! └──┴──┴─┴─┴─┴────────╱┄╱─────────┴────────╱┄╱──────────┘
//!  DD SS L G A       Payload              Metadata
//! ```
//!
//! Where the fields are:
//!
//! | Symbol   | Length   | Description              | Range         |
//! |----------|----------|--------------------------|---------------|
//! | DD       | 2 bytes  | Destination (Big-endian) | 0x0000-0xFFFF |
//! | SS       | 2 bytes  | Source (Big-endian)      | 0x0000-0xFFFF |
//! | L        | 1 byte   | Payload length           | 0x00-0x??     |
//! | G        | 1 byte   | AM group                 | 0x00-0xFF     |
//! | A        | 1 byte   | AM ID                    | 0x00-0xFF     |
//! | Payload  | variable | Data payload             |               |
//! | Metadata | variable | Metadata                 |               |
//!
//! ## More information
//!
//! There is some more information in the [Serial protocol][1] documentation.
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/Serial-protocol
use std::convert::{TryFrom, TryInto};
use std::fmt;

use super::Dispatcher;
use crate::{Bytes, Event};

mod dispatcher;
mod receiver;

pub use dispatcher::{AMDispatcher, AMDispatcherBuilder};
pub use receiver::AMReceiver;

const DEST_START: usize = 0;
const SRC_START: usize = 2;
const LENGTH_START: usize = 4;
const GROUP_START: usize = 5;
const ID_START: usize = 6;
const PAYLOAD_START: usize = 7;
const MINIMUM_LENGTH: u8 = 7;

/// An ActiveMessage packet
///
/// # Example
///
/// ```rust
/// use moteconnection::dispatcher::am::Message;
/// use std::convert::TryFrom;
///
/// let input = vec![0xFF, 0xFF, 0x00, 0x01, 0x00, 0x22, 0x00];
/// let message = Message::try_from(input).unwrap();
/// assert_eq!(message.dest, 0xFFFF);
/// assert_eq!(message.src, 0x0001);
/// assert_eq!(message.length, 0);
/// assert_eq!(message.group, 0x22);
/// assert_eq!(message.id, 0);
/// assert_eq!(message.payload, Vec::new());
/// ```
#[derive(Debug, Clone)]
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
    /// An optional metadata component
    pub metadata: Vec<u8>,
}

/// Represents the types of errors that constructiong a `Message` can produce.
#[derive(Debug)]
pub enum MessageParseError {
    /// Indicates that the supplied byte vector was too large.
    VecTooLarge {
        /// A verbose message for the developer
        message: String,
        /// The size that was encontered
        length: usize,
    },
    /// Indicates that the supplied byte vector was too small.
    VecTooSmall {
        /// A verbose message for the developer
        message: String,
        /// The size that was encontered
        length: usize,
    },
    /// Indicates that the payload length did not match the length of the
    /// actual payload.
    PayloadSizeMismatch {
        /// A verbose message for the developer
        message: String,
        /// The size that was stated
        stated: u8,
        /// The size that was discovered
        actual: u8,
    },
}

impl TryFrom<Vec<u8>> for Message {
    type Error = MessageParseError;
    fn try_from(data: Vec<u8>) -> Result<Message, Self::Error> {
        use MessageParseError::*;

        let length: u8 = match data.len().try_into() {
            Ok(v) => v,
            Err(_) => {
                return Err(VecTooLarge {
                    message: format!(
                        "The byte vector must not be larger than 255 bytes. Found {} bytes.",
                        data.len()
                    ),
                    length: data.len(),
                });
            }
        };
        if length < MINIMUM_LENGTH {
            return Err(VecTooSmall {
                message: format!(
                    "The byte vector must be at least 8 bytes long!. Found {} bytes.",
                    length
                ),
                length: length as usize,
            });
        }
        if length < data[LENGTH_START] + MINIMUM_LENGTH {
            return Err(PayloadSizeMismatch {
                message: format!(
                    "The byte vector reports payload length of {} but is actually {}",
                    data[LENGTH_START],
                    length - MINIMUM_LENGTH
                ),
                stated: data[LENGTH_START],
                actual: length - MINIMUM_LENGTH,
            });
        }

        let payload_end = (data[LENGTH_START] + MINIMUM_LENGTH) as usize;
        // TODO(Kaarel): Copy used when could transfer ownership
        Ok(Message {
            dest: u16::from_be_bytes([data[DEST_START], data[DEST_START + 1]]),
            src: u16::from_be_bytes([data[SRC_START], data[SRC_START + 1]]),
            length: data[LENGTH_START],
            group: data[GROUP_START],
            id: data[ID_START],
            payload: data[PAYLOAD_START..payload_end].to_vec(),
            metadata: data[payload_end..].to_vec(),
        })
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        let mut result = Vec::with_capacity(
            usize::from(MINIMUM_LENGTH) + self.payload.len() + self.metadata.len(),
        );
        result.extend(
            [].iter()
                .chain(self.dest.to_be_bytes().iter())
                .chain(self.src.to_be_bytes().iter())
                .chain([self.length, self.group, self.id].iter())
                .chain(self.payload.iter())
                .chain(self.metadata.iter()),
        );
        result
    }
}

impl Into<Event<Bytes>> for Message {
    fn into(self) -> Event<Bytes> {
        Event::Data(self.into())
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{:02X}}}{:04X}->{:04X}[{:02X}]{:>3}: {} {}",
            self.group,
            self.src,
            self.dest,
            self.id,
            self.payload.len(),
            self.payload
                .iter()
                .fold(String::new(), |string, byte| format!(
                    "{}{:02X}",
                    string, byte
                )),
            self.metadata
                .iter()
                .fold(String::new(), |string, byte| {
                    format!("{}{:02X}", string, byte)
                })
        )
    }
}

#[cfg(test)]
mod test {
    use super::MessageParseError::*;
    use super::*;

    #[test]
    fn test_message_empty_payload_no_metadata() {
        let input = vec![0xFF, 0xFF, 0x00, 0x01, 0x00, 0x22, 0x00];
        let message = Message::try_from(input).unwrap();
        assert_eq!(message.dest, 0xFFFF);
        assert_eq!(message.src, 0x0001);
        assert_eq!(message.length, 0);
        assert_eq!(message.group, 0x22);
        assert_eq!(message.id, 0);
        assert_eq!(message.payload, Vec::new());
        assert_eq!(message.metadata, Vec::new());
    }

    #[test]
    fn test_message_empty_payload_metadata() {
        let input = vec![0xFF, 0xFF, 0x00, 0x01, 0x00, 0x22, 0x00, 0x01, 0x02];
        let message = Message::try_from(input).unwrap();
        assert_eq!(message.dest, 0xFFFF);
        assert_eq!(message.src, 0x0001);
        assert_eq!(message.length, 0);
        assert_eq!(message.group, 0x22);
        assert_eq!(message.id, 0);
        assert_eq!(message.payload, Vec::new());
        assert_eq!(message.metadata, vec![0x01, 0x02]);
    }

    #[test]
    fn test_message_existing_payload_no_metadata() {
        let input = vec![
            0xFF, 0xFF, 0x00, 0x01, 0x04, 0x22, 0x00, 0x00, 0x01, 0x02, 0x03,
        ];
        let message = Message::try_from(input).unwrap();
        assert_eq!(message.dest, 0xFFFF);
        assert_eq!(message.src, 0x0001);
        assert_eq!(message.length, 4);
        assert_eq!(message.group, 0x22);
        assert_eq!(message.id, 0);
        assert_eq!(message.payload, vec![0, 1, 2, 3]);
        assert_eq!(message.metadata, Vec::new());
    }

    #[test]
    fn test_message_existing_payload_metadata() {
        let input = vec![
            0xFF, 0xFF, 0x00, 0x01, 0x04, 0x22, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        ];
        let message = Message::try_from(input).unwrap();
        assert_eq!(message.dest, 0xFFFF);
        assert_eq!(message.src, 0x0001);
        assert_eq!(message.length, 4);
        assert_eq!(message.group, 0x22);
        assert_eq!(message.id, 0);
        assert_eq!(message.payload, vec![0, 1, 2, 3]);
        assert_eq!(message.metadata, vec![4, 5]);
    }

    #[test]
    fn test_message_too_large_payload() {
        let input = vec![0xFF, 0xFF, 0x00, 0x01, 0x00, 0x22, 0x00, 0x01];
        let message = Message::try_from(input).unwrap();
        assert_eq!(message.dest, 0xFFFF);
        assert_eq!(message.src, 0x0001);
        assert_eq!(message.length, 0);
        assert_eq!(message.group, 0x22);
        assert_eq!(message.id, 0);
        assert_eq!(message.payload, Vec::new());
        assert_eq!(message.metadata, vec![1]);
    }

    #[test]
    fn test_message_too_small_payload() {
        let input = vec![0xFF, 0xFF, 0x00, 0x01, 0x01, 0x22, 0x00];
        match Message::try_from(input).unwrap_err() {
            PayloadSizeMismatch { stated, actual, .. } => {
                assert_eq!(stated, 1);
                assert_eq!(actual, 0);
            }
            e => {
                panic!("Unexpected error! {:?}", e);
            }
        }
    }

    #[test]
    fn test_message_too_large_data() {
        let input = vec![0x00; 65535];
        match Message::try_from(input).unwrap_err() {
            VecTooLarge { length, .. } => {
                assert_eq!(length, 65535);
            }
            e => {
                panic!("Unexpected error! {:?}", e);
            }
        }
    }

    #[test]
    fn test_message_too_small_data() {
        let input = vec![0x00; 2];
        match Message::try_from(input).unwrap_err() {
            VecTooSmall { length, .. } => {
                assert_eq!(length, 2);
            }
            e => {
                panic!("Unexpected error! {:?}", e);
            }
        }
    }

    #[test]
    fn test_message_into_bytes() {
        let input = vec![
            0xFF, 0xFF, 0x00, 0x01, 0x04, 0x22, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        ];
        let message = Message::try_from(input.to_owned()).unwrap();
        let output: Vec<u8> = message.into();

        assert_eq!(input, output);
    }
}
