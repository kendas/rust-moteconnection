//! The ActiveMessage dispatch scheme is used for sending addressed packets.
//!
//! The ActiveMessage dispatch scheme has the dispatch byte of 0x00.
//! The general structure is the following:
//!
//! TODO(Kaarel): Dispatch byte
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
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use super::raw::RawDispatcher;
use super::{Dispatcher, DispatcherBuilder};

/// Allows the receiving and sending of ActiveMessage packets.
pub struct AMReceiver {
    /// The sender for ActiveMessage packets.
    pub tx: Sender<Message>,
    /// The receiver for ActiveMessage packets.
    pub rx: Receiver<Message>,
    handle: Arc<Mutex<AMReceiverHandle>>,
}

struct AMReceiverHandle {
    tx: Sender<Message>,
    rx: Receiver<Message>,
}

/// Implements the `Dispatcher` trait for the ActiveMessage dispatch scheme.
///
/// TODO(Kaarel)
pub struct AMDispatcher {
    dispatcher: Dispatcher,
    addr: u16,
    group: u8,
    receivers: HashMap<u8, Arc<Mutex<AMReceiverHandle>>>,
    default_receiver: Option<Arc<Mutex<AMReceiverHandle>>>,
    snoopers: HashMap<u8, Arc<Mutex<AMReceiverHandle>>>,
    default_snooper: Option<Arc<Mutex<AMReceiverHandle>>>,
}

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
#[derive(Debug)]
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

impl AMReceiver {
    /// Creates a new instance of an AMReceiver.
    pub fn new() -> AMReceiver {
        AMReceiver::default()
    }

    fn get_handle(&self) -> Arc<Mutex<AMReceiverHandle>> {
        self.handle.clone()
    }
}

impl Default for AMReceiver {
    fn default() -> AMReceiver {
        let (handle_tx, rx) = mpsc::channel();
        let (tx, handle_rx) = mpsc::channel();
        AMReceiver {
            tx,
            rx,
            handle: Arc::new(Mutex::new(AMReceiverHandle {
                tx: handle_tx,
                rx: handle_rx,
            })),
        }
    }
}

impl AMDispatcher {
    /// Creates a new `AMDispatcher` that listens on the default 0x22 group.
    pub fn new(addr: u16) -> Self {
        AMDispatcher::with_group(addr, 0x22)
    }

    /// Creates a new `AMDispatcher` that listens on the group provided.
    pub fn with_group(addr: u16, group: u8) -> Self {
        AMDispatcher {
            dispatcher: RawDispatcher::new(0x00).create(),
            addr,
            group,
            receivers: HashMap::new(),
            default_receiver: None,
            snoopers: HashMap::new(),
            default_snooper: None,
        }
    }

    /// Registers a receiver as the receiver for a specific AM ID.
    ///
    /// Receivers handle all packets that are intended for the
    /// dispatcher address or the broadcast address.
    pub fn register_receiver(&mut self, receiver: &AMReceiver, id: u8) -> &AMDispatcher {
        self.receivers.insert(id, receiver.get_handle());
        self
    }

    /// Registers a receiver as the receiver for a specific AM ID.
    ///
    /// Receivers handle all packets that are intended for the
    /// dispatcher address or the broadcast address.
    ///
    /// The default receiver handles all packets that have not been handled
    /// by another receiver.
    pub fn register_default_receiver(&mut self, receiver: &AMReceiver) -> &AMDispatcher {
        self.default_receiver = Some(receiver.get_handle());
        self
    }

    /// Registers a receiver as the snooper for a specific AM ID.
    ///
    /// Snoopers handle all packets that are not intended for the
    /// dispatcher address or the broadcast address.
    pub fn register_snooper(&mut self, receiver: &AMReceiver, id: u8) -> &AMDispatcher {
        self.snoopers.insert(id, receiver.get_handle());
        self
    }

    /// Registers a receiver as the default snooper.
    ///
    /// Snoopers handle all packets that are not intended for the
    /// dispatcher address or the broadcast address.
    ///
    /// The default snooper handles all packets that have not been handled
    /// by another snooper.
    pub fn register_default_snooper(&mut self, receiver: &AMReceiver) -> &AMDispatcher {
        self.default_snooper = Some(receiver.get_handle());
        self
    }
}

impl DispatcherBuilder for AMDispatcher {
    fn create(self) -> Dispatcher {
        self.dispatcher
    }
}

// impl Default for AMDispatcher {
//     fn default() -> Self {
//         AMDispatcher {
//             dispatcher: RawDispatcher::new(0x00),
//             receivers: Arc::new(Mutex::new(HashMap::new())),
//         }
//     }
// }

const DEST_START: usize = 0;
const SRC_START: usize = 2;
const LENGTH_START: usize = 4;
const GROUP_START: usize = 5;
const ID_START: usize = 6;
const PAYLOAD_START: usize = 7;
const MINIMUM_LENGTH: u8 = 7;

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
