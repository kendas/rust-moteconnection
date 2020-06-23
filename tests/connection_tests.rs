extern crate moteconnection;

use std::convert::{TryFrom, TryInto};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};

use moteconnection::dispatcher::am::{AMDispatcher, AMReceiver, Message};
use moteconnection::dispatcher::raw::RawDispatcher;
use moteconnection::dispatcher::DispatcherBuilder;
use moteconnection::ConnectionBuilder;

type Bytes = Vec<u8>;

struct TestServer {
    listener: TcpListener,
    stream: Option<TcpStream>,
}

impl TestServer {
    fn new(addr: &str) -> TestServer {
        TestServer {
            listener: TcpListener::bind(addr).unwrap(),
            stream: None,
        }
    }

    fn accept(&mut self) {
        self.stream = Some(self.listener.incoming().next().unwrap().unwrap());
    }

    fn do_handshake(&mut self) {
        self.stream.as_ref().unwrap().write_all(b"U ").unwrap();
    }

    fn write(&mut self, data: &[u8]) {
        self.stream
            .as_ref()
            .unwrap()
            .write_all(&[u8::try_from(data.len()).unwrap()])
            .unwrap();
        self.stream.as_ref().unwrap().write_all(data).unwrap();
    }

    fn read(&mut self, length: usize) -> Bytes {
        let mut packet_buf = vec![0; length];
        self.stream
            .as_ref()
            .unwrap()
            .read_exact(&mut packet_buf[..])
            .unwrap();
        packet_buf
    }

    fn shutdown(self) {
        self.stream.unwrap().shutdown(Shutdown::Both).unwrap();
    }
}

#[test]
#[ignore]
fn test_raw_packet_connection() {
    let connection_string = String::from("sf@localhost:9200");
    let mut server = TestServer::new("localhost:9200");

    let dispatcher = RawDispatcher::new(0x01).create();
    let connection = ConnectionBuilder::new(connection_string)
        .register_dispatcher(&dispatcher)
        .start()
        .unwrap();

    server.accept();
    server.do_handshake();

    let mut input = vec![1, 2, 3];

    server.write(&mut input[..]);

    let output = dispatcher.rx.try_recv().unwrap();

    connection.stop().unwrap();
    server.shutdown();

    assert_eq!(input, output);
}

#[test]
#[ignore]
fn test_am_default_receiver_connection() {
    let connection_string = String::from("sf@localhost:9201");
    let mut server = TestServer::new("localhost:9201");

    let mut dispatcher_builder = AMDispatcher::new(0x1234);
    let receiver = AMReceiver::new();
    dispatcher_builder.register_default_receiver(&receiver);
    let dispatcher = dispatcher_builder.create();

    let connection = ConnectionBuilder::new(connection_string)
        .register_dispatcher(&dispatcher)
        .start()
        .unwrap();

    server.accept();
    server.do_handshake();

    let mut input = vec![0x00, 0x12, 0x34, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    server.write(&mut input[..]);

    let output: Message = receiver.rx.try_recv().unwrap().try_into().unwrap();

    connection.stop().unwrap();
    server.shutdown();

    assert_eq!(output.dest, 0x1234);
    assert_eq!(output.src, 0x0001);
    assert_eq!(output.length, 0x01);
    assert_eq!(output.group, 0x22);
    assert_eq!(output.id, 0x02);
    assert_eq!(output.payload, vec![0xff]);
    assert_eq!(output.metadata, vec![]);
}

#[test]
#[ignore]
fn test_am_receiver_connection() {
    let connection_string = String::from("sf@localhost:9202");
    let mut server = TestServer::new("localhost:9202");

    let mut dispatcher_builder = AMDispatcher::new(0x1234);
    let receiver = AMReceiver::new();
    dispatcher_builder.register_receiver(&receiver, 0x02);
    let dispatcher = dispatcher_builder.create();

    let connection = ConnectionBuilder::new(connection_string)
        .register_dispatcher(&dispatcher)
        .start()
        .unwrap();

    server.accept();
    server.do_handshake();

    let mut input = vec![0x00, 0x12, 0x34, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    server.write(&mut input[..]);

    let output: Message = receiver.rx.try_recv().unwrap().try_into().unwrap();

    connection.stop().unwrap();
    server.shutdown();

    assert_eq!(output.dest, 0x1234);
    assert_eq!(output.src, 0x0001);
    assert_eq!(output.length, 0x01);
    assert_eq!(output.group, 0x22);
    assert_eq!(output.id, 0x02);
    assert_eq!(output.payload, vec![0xff]);
    assert_eq!(output.metadata, vec![]);
}

#[test]
#[ignore]
fn test_am_default_snooper_connection() {
    let connection_string = String::from("sf@localhost:9203");
    let mut server = TestServer::new("localhost:9203");

    let mut dispatcher_builder = AMDispatcher::new(0x1234);
    let receiver = AMReceiver::new();
    dispatcher_builder.register_default_snooper(&receiver);
    let dispatcher = dispatcher_builder.create();

    let connection = ConnectionBuilder::new(connection_string)
        .register_dispatcher(&dispatcher)
        .start()
        .unwrap();

    server.accept();
    server.do_handshake();

    let mut input = vec![0x00, 0x12, 0x35, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    server.write(&mut input[..]);

    let output: Message = receiver.rx.try_recv().unwrap().try_into().unwrap();

    connection.stop().unwrap();
    server.shutdown();

    assert_eq!(output.dest, 0x1235);
    assert_eq!(output.src, 0x0001);
    assert_eq!(output.length, 0x01);
    assert_eq!(output.group, 0x22);
    assert_eq!(output.id, 0x02);
    assert_eq!(output.payload, vec![0xff]);
    assert_eq!(output.metadata, vec![]);
}

#[test]
#[ignore]
fn test_am_snooper_connection() {
    let connection_string = String::from("sf@localhost:9204");
    let mut server = TestServer::new("localhost:9204");

    let mut dispatcher_builder = AMDispatcher::new(0x1234);
    let receiver = AMReceiver::new();
    dispatcher_builder.register_snooper(&receiver, 0x02);
    let dispatcher = dispatcher_builder.create();

    let connection = ConnectionBuilder::new(connection_string)
        .register_dispatcher(&dispatcher)
        .start()
        .unwrap();

    server.accept();
    server.do_handshake();

    let mut input = vec![0x00, 0x12, 0x35, 0x00, 0x01, 0x01, 0x22, 0x02, 0xff];

    server.write(&mut input[..]);

    let output: Message = receiver.rx.try_recv().unwrap().try_into().unwrap();

    connection.stop().unwrap();
    server.shutdown();

    assert_eq!(output.dest, 0x1235);
    assert_eq!(output.src, 0x0001);
    assert_eq!(output.length, 0x01);
    assert_eq!(output.group, 0x22);
    assert_eq!(output.id, 0x02);
    assert_eq!(output.payload, vec![0xff]);
    assert_eq!(output.metadata, vec![]);
}
