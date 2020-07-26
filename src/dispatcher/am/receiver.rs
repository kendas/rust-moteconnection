use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use uuid::Uuid;

use super::Message;

pub(super) struct AMReceiverHandle {
    pub tx: Sender<Message>,
}

/// Allows the receiving and sending of ActiveMessage packets.
pub struct AMReceiver {
    /// The receiver for ActiveMessage packets.
    pub rx: Receiver<Message>,
    id: Uuid,
    handle: Option<AMReceiverHandle>,
}

impl AMReceiver {
    /// Creates a new instance of an AMReceiver.
    pub fn new() -> AMReceiver {
        Default::default()
    }

    /// Returns the unique ID of this receiver.
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub(super) fn get_handle(&mut self) -> AMReceiverHandle {
        self.handle.take().unwrap()
    }
}

impl Default for AMReceiver {
    fn default() -> AMReceiver {
        let (handle_tx, rx) = mpsc::channel();
        AMReceiver {
            rx,
            id: Uuid::new_v4(),
            handle: Some(AMReceiverHandle { tx: handle_tx }),
        }
    }
}
