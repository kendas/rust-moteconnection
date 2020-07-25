use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use super::Message;

pub(super) struct AMReceiverHandle {
    pub tx: Sender<Message>,
    pub rx: Receiver<Message>,
}

/// Allows the receiving and sending of ActiveMessage packets.
pub struct AMReceiver {
    /// The sender for ActiveMessage packets.
    pub tx: Sender<Message>,
    /// The receiver for ActiveMessage packets.
    pub rx: Receiver<Message>,
    handle: Option<AMReceiverHandle>,
}

impl AMReceiver {
    /// Creates a new instance of an AMReceiver.
    pub fn new() -> AMReceiver {
        AMReceiver::default()
    }

    pub(super) fn get_handle(&mut self) -> AMReceiverHandle {
        self.handle.take().unwrap()
    }
}

impl Default for AMReceiver {
    fn default() -> AMReceiver {
        let (handle_tx, rx) = mpsc::channel();
        let (tx, handle_rx) = mpsc::channel();
        AMReceiver {
            tx,
            rx,
            handle: Some(AMReceiverHandle {
                tx: handle_tx,
                rx: handle_rx,
            }),
        }
    }
}
