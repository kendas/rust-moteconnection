use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::Builder;

use super::receiver::{AMReceiver, AMReceiverHandle};
use super::{Dispatcher, Message};
use crate::dispatcher::{DispatcherHandle, Event};

/// Implements the `Dispatcher` trait for the ActiveMessage dispatch scheme.
///
/// TODO(Kaarel)
pub struct AMDispatcher {
    dispatch_byte: u8,
    /// The channel to send data to the device.
    pub tx: Sender<Event>,
    handle: Option<DispatcherHandle>,
}

/// A builder for the AMDispatcher struct
pub struct AMDispatcherBuilder {
    addr: u16,
    group: u8,

    receivers: HashMap<u8, AMReceiverHandle>,
    default_receiver: Option<AMReceiverHandle>,

    snoopers: HashMap<u8, AMReceiverHandle>,
    default_snooper: Option<AMReceiverHandle>,
}

struct ConnectionWorker {
    rx: Receiver<Event>,

    addr: u16,
    group: u8,

    receivers: HashMap<u8, AMReceiverHandle>,
    default_receiver: Option<AMReceiverHandle>,

    snoopers: HashMap<u8, AMReceiverHandle>,
    default_snooper: Option<AMReceiverHandle>,
}

impl AMDispatcherBuilder {
    /// Creates a new `AMDispatcher` that listens on the default 0x22 group.
    pub fn new(addr: u16) -> AMDispatcherBuilder {
        AMDispatcherBuilder::with_group(addr, 0x22)
    }

    /// Creates a new `AMDispatcher` that listens on the group provided.
    pub fn with_group(addr: u16, group: u8) -> AMDispatcherBuilder {
        AMDispatcherBuilder {
            addr,
            group,
            receivers: HashMap::new(),
            default_receiver: None,
            snoopers: HashMap::new(),
            default_snooper: None,
        }
    }

    /// Creates the AMDispatcher.
    pub fn create(self) -> AMDispatcher {
        AMDispatcher::new(
            0x00,
            self.addr,
            self.group,
            self.receivers,
            self.default_receiver,
            self.snoopers,
            self.default_snooper,
        )
    }

    /// Registers a receiver as the receiver for a specific AM ID.
    ///
    /// Receivers handle all packets that are intended for the
    /// dispatcher address or the broadcast address.
    pub fn register_receiver(&mut self, receiver: &mut AMReceiver, id: u8) -> &AMDispatcherBuilder {
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
    pub fn register_default_receiver(&mut self, receiver: &mut AMReceiver) -> &AMDispatcherBuilder {
        self.default_receiver = Some(receiver.get_handle());
        self
    }

    /// Registers a receiver as the snooper for a specific AM ID.
    ///
    /// Snoopers handle all packets that are not intended for the
    /// dispatcher address or the broadcast address.
    pub fn register_snooper(&mut self, receiver: &mut AMReceiver, id: u8) -> &AMDispatcherBuilder {
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
    pub fn register_default_snooper(&mut self, receiver: &mut AMReceiver) -> &AMDispatcherBuilder {
        self.default_snooper = Some(receiver.get_handle());
        self
    }
}

impl AMDispatcher {
    fn new(
        dispatch_byte: u8,
        addr: u16,
        group: u8,
        receivers: HashMap<u8, AMReceiverHandle>,
        default_receiver: Option<AMReceiverHandle>,
        snoopers: HashMap<u8, AMReceiverHandle>,
        default_snooper: Option<AMReceiverHandle>,
    ) -> AMDispatcher {
        let (tx, connection_rx) = mpsc::channel();
        let (connection_tx, rx) = mpsc::channel();

        let join_handle = Builder::new()
            .name(format!("am-dispatcher-{}", dispatch_byte))
            .spawn(move || {
                ConnectionWorker {
                    rx,
                    addr,
                    group,
                    receivers,
                    default_receiver,
                    snoopers,
                    default_snooper,
                }
                .start();
            })
            .unwrap();

        let stopper_tx = connection_tx.clone();

        AMDispatcher {
            dispatch_byte,
            tx,
            handle: Some(DispatcherHandle::with_stopper(
                connection_tx,
                connection_rx,
                Box::new(move || {
                    if stopper_tx.send(Event::Stop).is_err() {
                        return Err("Dispatcher already stopped!");
                    }
                    if join_handle.join().is_err() {
                        return Err("Unable to join the AMDispatcher!");
                    }
                    Ok(())
                }),
            )),
        }
    }
}

impl Dispatcher for AMDispatcher {
    fn dispatch_byte(&self) -> u8 {
        self.dispatch_byte
    }

    fn get_handle(&mut self) -> DispatcherHandle {
        self.handle.take().unwrap()
    }
}

impl ConnectionWorker {
    fn start(self) {
        while let Ok(v) = self.rx.recv() {
            match v {
                Event::Data(data) => {
                    match Message::try_from(data) {
                        Ok(message) => self.handle_message(message),
                        Err(_e) => {
                            // TODO(Kaarel): Log malformed packet.
                        }
                    }
                }
                Event::Stop => {
                    break;
                }
                e => panic!(format!("Unexpected event {:?}", e)),
            };
        }
    }

    fn handle_message(&self, message: Message) {
        if message.group == self.group {
            if let Some(receiver) = self.select_receiver(message.dest, message.id) {
                receiver.tx.send(message).unwrap();
            }
        }
    }

    fn select_receiver(&self, dest: u16, am_id: u8) -> Option<&AMReceiverHandle> {
        if [0xFFFF, self.addr].contains(&dest) {
            self.receivers
                .get(&am_id)
                .or_else(|| self.default_receiver.as_ref())
        } else {
            self.snoopers
                .get(&am_id)
                .or_else(|| self.default_snooper.as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use super::*;

    #[test]
    fn test_select_receiver_valid_addr_exact_receiver() {
        let (_, rx) = mpsc::channel();
        let mut receiver = AMReceiver::new();
        let handle = receiver.get_handle();
        let worker = ConnectionWorker {
            rx,
            addr: 0x0010,
            group: 0x02,
            receivers: HashMap::from_iter(vec![(0x01, handle)].into_iter()),
            default_receiver: None,
            snoopers: HashMap::new(),
            default_snooper: None,
        };

        let selected = worker.select_receiver(0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_valid_addr_default_receiver() {
        let (_, rx) = mpsc::channel();
        let mut receiver = AMReceiver::new();
        let handle = receiver.get_handle();
        let worker = ConnectionWorker {
            rx,
            addr: 0x0010,
            group: 0x02,
            receivers: HashMap::new(),
            default_receiver: Some(handle),
            snoopers: HashMap::new(),
            default_snooper: None,
        };

        let selected = worker.select_receiver(0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_valid_addr_no_receiver() {
        let (_, rx) = mpsc::channel();
        let mut receiver = AMReceiver::new();
        let handle = receiver.get_handle();
        let worker = ConnectionWorker {
            rx,
            addr: 0x0010,
            group: 0x02,
            receivers: HashMap::from_iter(vec![(0x01, handle)].into_iter()),
            default_receiver: None,
            snoopers: HashMap::new(),
            default_snooper: None,
        };

        let selected = worker.select_receiver(0x0010, 0x02);
        assert_eq!(selected.is_none(), true);
    }

    #[test]
    fn test_select_receiver_invalid_addr_exact_snooper() {
        let (_, rx) = mpsc::channel();
        let mut receiver = AMReceiver::new();
        let handle = receiver.get_handle();
        let worker = ConnectionWorker {
            rx,
            addr: 0x0011,
            group: 0x02,
            receivers: HashMap::new(),
            default_receiver: None,
            snoopers: HashMap::from_iter(vec![(0x01, handle)].into_iter()),
            default_snooper: None,
        };

        let selected = worker.select_receiver(0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_invalid_addr_default_snooper() {
        let (_, rx) = mpsc::channel();
        let mut receiver = AMReceiver::new();
        let handle = receiver.get_handle();
        let worker = ConnectionWorker {
            rx,
            addr: 0x0011,
            group: 0x02,
            receivers: HashMap::new(),
            default_receiver: None,
            snoopers: HashMap::new(),
            default_snooper: Some(handle),
        };

        let selected = worker.select_receiver(0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_invalid_addr_no_snooper() {
        let (_, rx) = mpsc::channel();
        let mut receiver = AMReceiver::new();
        let handle = receiver.get_handle();
        let worker = ConnectionWorker {
            rx,
            addr: 0x0011,
            group: 0x02,
            receivers: HashMap::from_iter(vec![(0x01, handle)].into_iter()),
            default_receiver: None,
            snoopers: HashMap::new(),
            default_snooper: None,
        };

        let selected = worker.select_receiver(0x0010, 0x02);
        assert_eq!(selected.is_none(), true);
    }
}
