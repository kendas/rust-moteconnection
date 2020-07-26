//! The connection module contains ...
//!
//! TODO(Kaarel)
//!
use std::collections::HashMap;
use std::convert::{From, TryFrom};
use std::iter::FromIterator;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{Builder, JoinHandle};
use std::time::Duration;

use regex::Regex;

use crate::dispatcher::{Dispatcher, DispatcherHandle, Event as DEvent};
use crate::transport::serialforwarder::SFBuilder;
use crate::transport::{Event as TEvent, TransportBuilder, Transport};

type DispatchTxs = HashMap<u8, Sender<DEvent>>;
type DispatchRxs = HashMap<u8, Receiver<DEvent>>;
type DispatchStoppers = HashMap<u8, Box<dyn FnOnce() -> Result<(), &'static str>>>;

/// The `Connection` struct manages a persistent connection with a radio module.
///
/// The available transports for the conenction with a radio module that
/// are provided are:
/// - serial
/// - serialforwarder
///
/// TODO(Kaarel): Usage
pub struct Connection {
    transport: Transport,
    control_tx: Sender<()>,
    dispatchers: DispatchStoppers,
    join_handle: JoinHandle<()>,
}

/// A builder for the connection struct
///
/// TODO(Kaarel): Usage
pub struct ConnectionBuilder {
    dispatchers: HashMap<u8, DispatcherHandle>,
    transport: Box<dyn TransportBuilder>,
}

struct TransportWorker {
    stop: Receiver<()>,
    timeout: Duration,
    rx: Receiver<TEvent>,
    txs: DispatchTxs,
}

struct DispatcherWorker {
    dispatch_byte: u8,
    stop: Receiver<()>,
    timeout: Duration,
    rx: Receiver<DEvent>,
    tx: Sender<TEvent>,
}

impl Connection {
    /// Constructs a new instance of the `Connection` struct.
    ///
    /// TODO(Kaarel): Usage
    pub fn new(
        mut transport_builder: Box<dyn TransportBuilder>,
        dispatchers: HashMap<u8, DispatcherHandle>,
    ) -> Result<Connection, String> {
        let (control_tx, control_rx) = mpsc::channel();

        let (txs, mut rxs, stoppers) = decompose_dispatch_handles(dispatchers);
        let mut transport = transport_builder.start();
        let transport_handle = transport.get_handle();
        let transport_tx = transport_handle.tx.clone();

        let join_handle = Builder::new()
            .name("connection".into())
            .spawn(move || {
                let mut handles: Vec<(Sender<()>, JoinHandle<()>)> = vec![];

                let (ctrl_tx, ctrl_rx) = mpsc::channel::<()>();
                let handle = Builder::new()
                    .name("connection-transport".into())
                    .spawn(move || {
                        TransportWorker {
                            stop: ctrl_rx,
                            timeout: Duration::from_millis(100),
                            rx: transport_handle.rx,
                            txs,
                        }
                        .start();
                    })
                    .unwrap();
                handles.push((ctrl_tx, handle));

                for (dispatch_byte, rx) in rxs.drain() {
                    let (ctrl_tx, ctrl_rx) = mpsc::channel::<()>();
                    let transport_tx = transport_tx.clone();
                    let join_handle = Builder::new()
                        .name(format!("connection-dispatcher-{}", dispatch_byte))
                        .spawn(move || {
                            DispatcherWorker {
                                dispatch_byte,
                                stop: ctrl_rx,
                                timeout: Duration::from_millis(100),
                                tx: transport_tx,
                                rx,
                            }
                            .start();
                        })
                        .unwrap();
                    handles.push((ctrl_tx, join_handle));
                }

                let result = control_rx.recv();
                if let Err(e) = result {
                    panic!(format!("Error {:?}", e))
                }
                for (tx, _) in &handles {
                    if tx.send(()).is_err() {
                        // TODO(Kaarel): log
                    }
                }
                for (_, handle) in handles {
                    if handle.join().is_err() {
                        // TODO(Kaarel): log
                    }
                }
            })
            .unwrap();

        Ok(Connection {
            transport,
            control_tx,
            dispatchers: stoppers,
            join_handle,
        })
    }

    /// Shuts down the connection.
    ///
    /// TODO(Kaarel): Usage
    pub fn stop(mut self) -> Result<(), &'static str> {
        let mut errors = self.transport.stop().is_err();

        for (_, stopper) in self.dispatchers.drain() {
            errors |= stopper().is_err();
        }

        errors |= self.control_tx.send(()).is_err();
        errors |= self.join_handle.join().is_err();

        if errors {
            Err("Problems stopping dispatchers!")
        } else {
            Ok(())
        }
    }
}

impl ConnectionBuilder {
    /// Creates a new ConnectionBuilder
    pub fn with_connection_string(connection_string: String) -> Result<ConnectionBuilder, String> {
        Ok(ConnectionBuilder::with_transport(
            ConnectionBuilder::build_transport(&connection_string)?,
        ))
    }

    /// Creates a new ConenctionBuilder using a pre-built transport
    pub fn with_transport(transport: Box<dyn TransportBuilder>) -> ConnectionBuilder {
        ConnectionBuilder {
            dispatchers: HashMap::new(),
            transport,
        }
    }

    /// Registers a new dispatcher for a dispatch byte.
    ///
    /// # Example
    ///
    /// ```rust
    /// use moteconnection::ConnectionBuilder;
    /// use moteconnection::dispatcher::am::{AMDispatcherBuilder, AMReceiver};
    ///
    /// let mut dispatcher = AMDispatcherBuilder::new(0x1234);
    /// let mut receiver = AMReceiver::new();
    /// dispatcher.register_default_snooper(&mut receiver);
    /// let mut dispatcher = dispatcher.create();
    ///
    /// let builder = ConnectionBuilder::with_connection_string(String::from("sf@localhost:9002"))
    ///     .unwrap()
    ///     .register_dispatcher(&mut dispatcher);
    /// ```
    pub fn register_dispatcher(mut self, dispatcher: &mut dyn Dispatcher) -> Self {
        let dispatch_byte = dispatcher.dispatch_byte();
        self.dispatchers
            .insert(dispatch_byte, dispatcher.get_handle());
        self
    }

    /// Establishes a new connection and returns the handler.
    ///
    /// # Example
    ///
    /// ```rust
    /// ```
    pub fn start(self) -> Result<Connection, String> {
        Ok(Connection::new(self.transport, self.dispatchers)?)
    }

    fn build_transport(connection_string: &str) -> Result<Box<dyn TransportBuilder>, String> {
        let re = Regex::new(r"^(sf|serial)@([^:]+(:\d+)?)$").unwrap();
        if re.is_match(connection_string) {
            let caps = re.captures(connection_string).unwrap();
            match caps.get(1).unwrap().as_str() {
                "sf" => match SFBuilder::try_from(String::from(caps.get(2).unwrap().as_str())) {
                    Ok(v) => Ok(Box::new(v)),
                    Err(e) => Err(format!(
                        "Error while deconstructing connection string: {}",
                        e
                    )),
                },
                "serial" => Err(String::from("The serial protocol is not implemented yet!")),
                protocol => Err(format!("Unknown protocol: {}", protocol)),
            }
        } else {
            Err(format!(
                "Malformed connection string: {}",
                connection_string
            ))
        }
    }
}

impl TransportWorker {
    fn start(&self) {
        while self.can_continue() {
            match self.rx.recv_timeout(self.timeout) {
                Ok(data) => self.handle_data(data),
                Err(e) => match e {
                    RecvTimeoutError::Timeout => {
                        continue;
                    }
                    RecvTimeoutError::Disconnected => {
                        break;
                    }
                },
            }
        }
    }

    fn can_continue(&self) -> bool {
        match self.stop.try_recv() {
            Ok(()) => false,
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => false,
        }
    }

    fn handle_data(&self, data: TEvent) {
        match data {
            TEvent::Data(message) => self.send(message),
            TEvent::Connected => {
                // TODO(Kaarel): Implement correct handling.
            }
            TEvent::Disconnected => {
                // TODO(Kaarel): Implement correct handling.
            }
            m => panic!(format!("Unknown message from the transport: {:?}", m)),
        }
    }

    fn send(&self, message: Vec<u8>) {
        if let Some(dispatcher) = self.txs.get(&message[0]) {
            dispatcher
                .send(DEvent::Data(Vec::from(&message[1..])))
                .unwrap();
        }
    }
}

impl DispatcherWorker {
    fn start(&self) {
        while self.can_continue() {
            match self.rx.recv_timeout(self.timeout) {
                Ok(data) => self.handle_data(data),
                Err(e) => match e {
                    RecvTimeoutError::Timeout => {
                        continue;
                    }
                    RecvTimeoutError::Disconnected => {
                        break;
                    }
                },
            };
        }
    }

    fn can_continue(&self) -> bool {
        match self.stop.try_recv() {
            Ok(()) => false,
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => false,
        }
    }

    fn handle_data(&self, data: DEvent) {
        match data {
            DEvent::Data(message) => self.send(message),
            e => {
                panic!(format!("Unknown event {:?}!", e));
            }
        }
    }

    fn send(&self, message: Vec<u8>) {
        self.tx
            .send(TEvent::Data(Vec::from_iter(
                vec![self.dispatch_byte]
                    .into_iter()
                    .chain(message.into_iter()),
            )))
            .unwrap();
    }
}

fn decompose_dispatch_handles(
    mut handles: HashMap<u8, DispatcherHandle>,
) -> (DispatchTxs, DispatchRxs, DispatchStoppers) {
    let mut txs = HashMap::new();
    let mut rxs = HashMap::new();
    let mut stoppers = HashMap::new();
    for (key, handle) in handles.drain() {
        let DispatcherHandle { tx, rx, stopper } = handle;
        txs.insert(key, tx);
        rxs.insert(key, rx);
        stoppers.insert(key, stopper);
    }
    (txs, rxs, stoppers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener};

    #[test]
    fn test_from_invalid_string() {
        let result = ConnectionBuilder::with_connection_string(String::from(
            "ser-f@no-valid:80",
        ));
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_from_sf_string_no_port() {
        const SERVER_ADDR: &str = "localhost:9002";

        let listener = TcpListener::bind(SERVER_ADDR).unwrap();

        let connection =
            ConnectionBuilder::with_connection_string(String::from("sf@localhost"))
                .unwrap()
                .start()
                .unwrap();

        let data = b"U ";
        let mut server_stream = listener.incoming().next().unwrap().unwrap();
        server_stream.write_all(data).unwrap();

        let mut buffer = [0, 0];
        server_stream.read_exact(&mut buffer).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();
        assert_eq!(&buffer, b"U ");

        connection.stop().unwrap();
    }

    #[test]
    fn test_from_sf_string_explicit_port() {
        const SERVER_ADDR: &str = "localhost:13111";

        let listener = TcpListener::bind(SERVER_ADDR).unwrap();

        let mut _connection =
            ConnectionBuilder::with_connection_string(format!("sf@{}", SERVER_ADDR))
                .unwrap()
                .start()
                .unwrap();

        let data = b"U ";
        let mut server_stream = listener.incoming().next().unwrap().unwrap();
        server_stream.write_all(data).unwrap();

        let mut buffer = [0, 0];
        server_stream.read_exact(&mut buffer).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();
        assert_eq!(&buffer, b"U ");

        _connection.stop().unwrap();
    }

    #[test]
    fn test_transport_worker_stop() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();

        let worker = TransportWorker {
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            txs: HashMap::new(),
        };

        stop_tx.send(()).unwrap();

        assert_eq!(worker.can_continue(), false);
    }

    #[test]
    fn test_transport_worker_data_event() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = TransportWorker {
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            txs: HashMap::from_iter(vec![(1, worker_tx)].into_iter()),
        };

        let data = TEvent::Data(vec![1, 2]);
        worker.handle_data(data);

        match rx.recv().unwrap() {
            DEvent::Data(output) => assert_eq!(output, vec![2]),
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }

    #[test]
    fn test_transport_worker_connected_event() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = TransportWorker {
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            txs: HashMap::from_iter(vec![(1, worker_tx)].into_iter()),
        };

        let data = TEvent::Connected;
        worker.handle_data(data);

        match rx.try_recv() {
            Err(TryRecvError::Empty) => {}
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }

    #[test]
    fn test_transport_worker_disconnected_event() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = TransportWorker {
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            txs: HashMap::from_iter(vec![(1, worker_tx)].into_iter()),
        };

        let data = TEvent::Disconnected;
        worker.handle_data(data);

        match rx.try_recv() {
            Err(TryRecvError::Empty) => {}
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }

    #[test]
    fn test_transport_worker_send_exists() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = TransportWorker {
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            txs: HashMap::from_iter(vec![(1, worker_tx)].into_iter()),
        };

        let data = vec![1, 2];
        worker.send(data);

        match rx.recv().unwrap() {
            DEvent::Data(output) => assert_eq!(output, vec![2]),
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }

    #[test]
    fn test_transport_worker_send_does_not_exist() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = TransportWorker {
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            txs: HashMap::from_iter(vec![(1, worker_tx)].into_iter()),
        };

        let data = vec![2, 2];
        worker.send(data);

        match rx.try_recv() {
            Err(TryRecvError::Empty) => {}
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }

    #[test]
    fn test_dispatcher_worker_stop() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, _) = mpsc::channel();

        let worker = DispatcherWorker {
            dispatch_byte: 1,
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            tx: worker_tx,
        };

        stop_tx.send(()).unwrap();

        assert_eq!(worker.can_continue(), false);
    }

    #[test]
    fn test_dispatcher_worker_data_event() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = DispatcherWorker {
            dispatch_byte: 1,
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            tx: worker_tx,
        };

        let data = DEvent::Data(vec![1, 2]);
        worker.handle_data(data);

        match rx.recv().unwrap() {
            TEvent::Data(output) => assert_eq!(output, vec![1, 1, 2]),
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }

    #[test]
    fn test_dispatcher_worker_send() {
        let (_, stop_rx) = mpsc::channel();
        let (_, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();

        let worker = DispatcherWorker {
            dispatch_byte: 1,
            stop: stop_rx,
            timeout: Duration::from_micros(0),
            rx: worker_rx,
            tx: worker_tx,
        };

        let data = vec![2, 2];
        worker.send(data);

        match rx.recv().unwrap() {
            TEvent::Data(output) => assert_eq!(output, vec![1, 2, 2]),
            e => panic!(format!("Unexpected output: {:?}", e)),
        }
    }
}
