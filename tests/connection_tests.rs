extern crate moteconnection;

use std::convert::{TryFrom, TryInto};
use std::time::Duration;

use moteconnection::dispatcher::am::{AMDispatcherBuilder, AMReceiver, Message};
use moteconnection::dispatcher::raw::RawDispatcher;
use moteconnection::dispatcher::Event as DEvent;
use moteconnection::transport::Event;
use moteconnection::ConnectionBuilder;

mod harness {
    use std::cell::RefCell;
    use std::convert::TryFrom;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::sync::mpsc::{Receiver, Sender};

    use moteconnection::transport::{Event, Transport, TransportBuilder, TransportHandle};
    use moteconnection::Bytes;

    pub struct TestServer {
        listener: TcpListener,
        stream: Option<TcpStream>,
    }

    impl TestServer {
        pub fn new(addr: &str) -> TestServer {
            TestServer {
                listener: TcpListener::bind(addr).unwrap(),
                stream: None,
            }
        }

        pub fn accept(&mut self) {
            self.stream = Some(self.listener.incoming().next().unwrap().unwrap());
        }

        pub fn do_handshake(&mut self) {
            self.stream.as_ref().unwrap().write_all(b"U ").unwrap();
        }

        pub fn write(&mut self, data: &[u8]) {
            self.stream
                .as_ref()
                .unwrap()
                .write_all(&[u8::try_from(data.len()).unwrap()])
                .unwrap();
            self.stream.as_ref().unwrap().write_all(data).unwrap();
        }

        pub fn read(&mut self, length: usize) -> Bytes {
            let mut packet_buf = vec![0; length];
            self.stream
                .as_ref()
                .unwrap()
                .read_exact(&mut packet_buf[..])
                .unwrap();
            packet_buf
        }

        pub fn shutdown(self) {
            self.stream.unwrap().shutdown(Shutdown::Both).unwrap();
        }
    }

    #[derive(Clone)]
    pub struct TestTransport {
        pub handle: Rc<RefCell<Option<TransportHandle>>>,
        pub internal_tx: Rc<Sender<Event>>,
        pub internal_rx: Rc<Receiver<Event>>,
    }

    impl TestTransport {
        pub fn new() -> TestTransport {
            let (tx, internal_rx) = mpsc::channel();
            let (internal_tx, rx) = mpsc::channel();
            TestTransport {
                handle: Rc::new(RefCell::new(Some(TransportHandle { tx, rx }))),
                internal_tx: Rc::new(internal_tx),
                internal_rx: Rc::new(internal_rx),
            }
        }
    }

    impl TransportBuilder for TestTransport {
        fn start(&self) -> Transport {
            let TransportHandle { tx, rx } = self.handle.borrow_mut().take().unwrap();
            Transport::new(tx, rx)
        }
    }
}

#[test]
fn test_tcp_connection() {
    let connection_string = String::from("sf@localhost:9200");
    let mut server = harness::TestServer::new("localhost:9200");

    let mut dispatcher = RawDispatcher::new(0x01);
    let connection = ConnectionBuilder::with_connection_string(connection_string)
        .unwrap()
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    server.accept();
    server.do_handshake();

    let mut input = vec![1, 2, 3];

    server.write(&mut input[..]);

    match dispatcher
        .rx
        .recv_timeout(Duration::from_millis(1000))
        .unwrap()
    {
        DEvent::Data(output) => assert_eq!(output[..], input[1..]),
        e => panic!(format!("Expected Event::Data, received {:?}", e)),
    }
    connection.stop().unwrap();
    server.shutdown();
}

#[test]
fn test_raw_packet_connection() {
    let transport = harness::TestTransport::new();

    let mut dispatcher = RawDispatcher::new(0x01);
    let connection = ConnectionBuilder::with_transport(Box::new(transport.clone()))
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    let input = vec![1, 2, 3];

    transport
        .internal_tx
        .send(Event::Data(input.clone()))
        .unwrap();

    match dispatcher.rx.recv_timeout(Duration::from_millis(1000)) {
        Ok(value) => match value {
            DEvent::Data(output) => assert_eq!(output[..], input[1..]),
            e => panic!(format!("Expected Event::Data, received {:?}", e)),
        },
        Err(e) => {
            connection.stop().unwrap();
            panic!(format!("Unexpected error on recv: {:?}", e));
        }
    }
    connection.stop().unwrap();
}

#[test]
fn test_am_default_receiver_connection() {
    let transport = harness::TestTransport::new();

    let mut dispatcher = AMDispatcherBuilder::new(0x1234);
    let mut receiver = AMReceiver::new();
    dispatcher.register_default_receiver(&mut receiver);
    let mut dispatcher = dispatcher.create();

    let connection = ConnectionBuilder::with_transport(Box::new(transport.clone()))
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    let input = vec![0x00, 0x12, 0x34, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    transport
        .internal_tx
        .send(Event::Data(input.clone()))
        .unwrap();

    match receiver.rx.recv_timeout(Duration::from_millis(1000)) {
        Ok(result) => {
            let output = Message::try_from(result).unwrap();

            connection.stop().unwrap();

            assert_eq!(output.dest, 0x1234);
            assert_eq!(output.src, 0x0001);
            assert_eq!(output.length, 0x01);
            assert_eq!(output.group, 0x22);
            assert_eq!(output.id, 0x02);
            assert_eq!(output.payload, vec![0xff]);
            assert_eq!(output.metadata, vec![]);
        }
        Err(e) => {
            connection.stop().unwrap();
            panic!(format!("Unexpected error on recv: {:?}", e));
        }
    };
}

#[test]
fn test_am_receiver_connection() {
    let transport = harness::TestTransport::new();

    let mut dispatcher = AMDispatcherBuilder::new(0x1234);
    let mut receiver = AMReceiver::new();
    dispatcher.register_receiver(&mut receiver, 0x02);
    let mut dispatcher = dispatcher.create();

    let connection = ConnectionBuilder::with_transport(Box::new(transport.clone()))
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    let input = vec![0x00, 0x12, 0x34, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    transport
        .internal_tx
        .send(Event::Data(input.clone()))
        .unwrap();

    match receiver.rx.recv_timeout(Duration::from_millis(1000)) {
        Ok(result) => {
            let output: Message = result.try_into().unwrap();

            connection.stop().unwrap();

            assert_eq!(output.dest, 0x1234);
            assert_eq!(output.src, 0x0001);
            assert_eq!(output.length, 0x01);
            assert_eq!(output.group, 0x22);
            assert_eq!(output.id, 0x02);
            assert_eq!(output.payload, vec![0xff]);
            assert_eq!(output.metadata, vec![]);
        }
        Err(e) => {
            connection.stop().unwrap();
            panic!(format!("Unexpected error on recv: {:?}", e));
        }
    };
}

#[test]
fn test_am_default_snooper_connection() {
    let transport = harness::TestTransport::new();

    let mut dispatcher = AMDispatcherBuilder::new(0x1234);
    let mut receiver = AMReceiver::new();
    dispatcher.register_default_snooper(&mut receiver);
    let mut dispatcher = dispatcher.create();

    let connection = ConnectionBuilder::with_transport(Box::new(transport.clone()))
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    let input = vec![0x00, 0x12, 0x35, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    transport
        .internal_tx
        .send(Event::Data(input.clone()))
        .unwrap();

    match receiver.rx.recv_timeout(Duration::from_millis(1000)) {
        Ok(result) => {
            let output: Message = result.try_into().unwrap();

            connection.stop().unwrap();

            assert_eq!(output.dest, 0x1235);
            assert_eq!(output.src, 0x0001);
            assert_eq!(output.length, 0x01);
            assert_eq!(output.group, 0x22);
            assert_eq!(output.id, 0x02);
            assert_eq!(output.payload, vec![0xff]);
            assert_eq!(output.metadata, vec![]);
        }
        Err(e) => {
            connection.stop().unwrap();
            panic!(format!("Unexpected error on recv: {:?}", e));
        }
    };
}

#[test]
fn test_am_snooper_connection() {
    let transport = harness::TestTransport::new();

    let mut dispatcher = AMDispatcherBuilder::new(0x1234);
    let mut receiver = AMReceiver::new();
    dispatcher.register_snooper(&mut receiver, 0x02);
    let mut dispatcher = dispatcher.create();

    let connection = ConnectionBuilder::with_transport(Box::new(transport.clone()))
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    let input = vec![0x00, 0x12, 0x35, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    transport
        .internal_tx
        .send(Event::Data(input.clone()))
        .unwrap();

    match receiver.rx.recv_timeout(Duration::from_millis(1000)) {
        Ok(result) => {
            let output: Message = result.try_into().unwrap();

            connection.stop().unwrap();

            assert_eq!(output.dest, 0x1235);
            assert_eq!(output.src, 0x0001);
            assert_eq!(output.length, 0x01);
            assert_eq!(output.group, 0x22);
            assert_eq!(output.id, 0x02);
            assert_eq!(output.payload, vec![0xff]);
            assert_eq!(output.metadata, vec![]);
        }
        Err(e) => {
            connection.stop().unwrap();
            panic!(format!("Unexpected error on recv: {:?}", e));
        }
    };
}

#[test]
fn test_am_connection_send() {
    let transport = harness::TestTransport::new();

    let mut dispatcher = AMDispatcherBuilder::new(0x1234);
    let mut receiver = AMReceiver::new();
    dispatcher.register_snooper(&mut receiver, 0x02);
    let mut dispatcher = dispatcher.create();

    let connection = ConnectionBuilder::with_transport(Box::new(transport.clone()))
        .register_dispatcher(&mut dispatcher)
        .start()
        .unwrap();

    let input = Message {
        dest: 0x1235,
        src: 0x0001,
        length: 0x01,
        group: 0x22,
        id: 0x02,
        payload: vec![0xff],
        metadata: vec![],
    };

    dispatcher.tx.send(input.clone().into()).unwrap();

    match transport
        .internal_rx
        .recv_timeout(Duration::from_millis(1000))
    {
        Ok(result) => match result {
            Event::Data(data) => {
                let dispatch_byte = data[0];
                let output: Message = data[1..].to_vec().try_into().unwrap();
                connection.stop().unwrap();

                assert_eq!(dispatch_byte, 0x00);
                assert_eq!(output.dest, 0x1235);
                assert_eq!(output.src, 0x0001);
                assert_eq!(output.length, 0x01);
                assert_eq!(output.group, 0x22);
                assert_eq!(output.id, 0x02);
                assert_eq!(output.payload, vec![0xff]);
                assert_eq!(output.metadata, vec![]);
            }
            e => {
                connection.stop().unwrap();
                panic!(format!("Unexpected message type: {:?}", e));
            }
        },
        Err(e) => {
            connection.stop().unwrap();
            panic!(format!("Unexpected error on recv: {:?}", e));
        }
    };
}
