use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::Builder;

use uuid::Uuid;

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

    receivers: ReceiverRegistry,
}

struct ReceiverRegistry {
    receivers: HashMap<u8, Uuid>,
    default_receiver: Option<Uuid>,

    snoopers: HashMap<u8, Uuid>,
    default_snooper: Option<Uuid>,

    handles: HashMap<Uuid, AMReceiverHandle>,
}

struct ConnectionWorker {
    rx: Receiver<Event>,

    addr: u16,
    group: u8,

    receivers: ReceiverRegistry,
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
            receivers: Default::default(),
        }
    }

    /// Creates the AMDispatcher.
    pub fn create(self) -> AMDispatcher {
        AMDispatcher::new(0x00, self.addr, self.group, self.receivers)
    }

    /// Registers a receiver as the receiver for a specific AM ID.
    ///
    /// Receivers handle all packets that are intended for the
    /// dispatcher address or the broadcast address.
    pub fn register_receiver(&mut self, receiver: &mut AMReceiver, id: u8) -> &AMDispatcherBuilder {
        self.receivers.register_receiver(receiver, id);
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
        self.receivers.register_default_receiver(receiver);
        self
    }

    /// Registers a receiver as the snooper for a specific AM ID.
    ///
    /// Snoopers handle all packets that are not intended for the
    /// dispatcher address or the broadcast address.
    pub fn register_snooper(&mut self, receiver: &mut AMReceiver, id: u8) -> &AMDispatcherBuilder {
        self.receivers.register_snooper(receiver, id);
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
        self.receivers.register_default_snooper(receiver);
        self
    }
}

impl AMDispatcher {
    fn new(dispatch_byte: u8, addr: u16, group: u8, receivers: ReceiverRegistry) -> AMDispatcher {
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

impl ReceiverRegistry {
    fn register_receiver(&mut self, receiver: &mut AMReceiver, id: u8) {
        self.handles
            .entry(receiver.id())
            .or_insert_with(|| receiver.get_handle());
        self.receivers.insert(id, receiver.id());
    }

    fn register_default_receiver(&mut self, receiver: &mut AMReceiver) {
        self.handles
            .entry(receiver.id())
            .or_insert_with(|| receiver.get_handle());
        self.default_receiver = Some(receiver.id());
    }

    fn register_snooper(&mut self, receiver: &mut AMReceiver, id: u8) {
        self.handles
            .entry(receiver.id())
            .or_insert_with(|| receiver.get_handle());
        self.snoopers.insert(id, receiver.id());
    }

    fn register_default_snooper(&mut self, receiver: &mut AMReceiver) {
        self.handles
            .entry(receiver.id())
            .or_insert_with(|| receiver.get_handle());
        self.default_snooper = Some(receiver.id());
    }

    fn select_receiver(&self, addr: u16, dest: u16, am_id: u8) -> Option<&AMReceiverHandle> {
        let id = if [0xFFFF, addr].contains(&dest) {
            self.receivers
                .get(&am_id)
                .or_else(|| self.default_receiver.as_ref())
        } else {
            self.snoopers
                .get(&am_id)
                .or_else(|| self.default_snooper.as_ref())
        };
        if let Some(id) = id {
            self.handles.get(&id)
        } else {
            None
        }
    }
}

impl Default for ReceiverRegistry {
    fn default() -> ReceiverRegistry {
        ReceiverRegistry {
            receivers: Default::default(),
            default_receiver: None,
            snoopers: Default::default(),
            default_snooper: None,
            handles: Default::default(),
        }
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
            if let Some(receiver) =
                self.receivers
                    .select_receiver(self.addr, message.dest, message.id)
            {
                receiver.tx.send(message).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_multi_use_receiver() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_default_receiver(&mut receiver);
        registry.register_default_snooper(&mut receiver);

        let selected = registry.select_receiver(0x0010, 0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_valid_addr_exact_receiver() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_receiver(&mut receiver, 0x01);

        let selected = registry.select_receiver(0x0010, 0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_valid_addr_default_receiver() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_default_receiver(&mut receiver);

        let selected = registry.select_receiver(0x0010, 0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_valid_addr_no_receiver() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_receiver(&mut receiver, 0x01);

        let selected = registry.select_receiver(0x0010, 0x0010, 0x02);
        assert_eq!(selected.is_none(), true);
    }

    #[test]
    fn test_select_receiver_invalid_addr_exact_snooper() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_snooper(&mut receiver, 0x01);

        let selected = registry.select_receiver(0x0011, 0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_invalid_addr_default_snooper() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_default_snooper(&mut receiver);

        let selected = registry.select_receiver(0x0011, 0x0010, 0x01);
        assert_eq!(selected.is_some(), true);
    }

    #[test]
    fn test_select_receiver_invalid_addr_no_snooper() {
        let mut receiver = AMReceiver::new();
        let mut registry: ReceiverRegistry = Default::default();
        registry.register_snooper(&mut receiver, 0x01);

        let selected = registry.select_receiver(0x0011, 0x0010, 0x02);
        assert_eq!(selected.is_none(), true);
    }
}
