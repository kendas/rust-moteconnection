//! The [serial][1] protocol implements the TinyOS serial
//! transport for packets.
//!
//! TODO(Kaarel): More
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/Serial-protocol
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::Builder;

use super::{Event, Transport, TransportBuilder};
use crate::Bytes;
use hdlc::HdlcCodec;
use packets::{Ack, AckPacket, NoAckPacket};

pub mod hdlc;
pub mod packets;

const ACKPACKET: u8 = 0x44;
const ACK: u8 = 0x43;
const NOACKPACKET: u8 = 0x45;

/// A builder object for the serial protocol `Transport`
pub struct SerialBuilder {
    addr: String,
}

struct SerialWorker {
    seq_num: u8,
    waiting_for_ack: Arc<Mutex<HashMap<u8, Bytes>>>,

    decoder: HdlcCodec,

    tx: Sender<Event>,
    loopback: Sender<Event>,
    // serial_port: ??,
}

struct ConnectionWorker<'a> {
    seq_num: u8,
    waiting_for_ack: Arc<Mutex<HashMap<u8, Bytes>>>,

    tx: Sender<Event>,
    rx: &'a Receiver<Event>,
    // serial_port: ??,
}

impl SerialBuilder {
    /// Creates a new `SerialBuilder`
    ///
    /// TODO(Kaarel): Usage
    pub fn new(addr: String) -> SerialBuilder {
        SerialBuilder { addr }
    }
}

impl TransportBuilder for SerialBuilder {
    /// Creates a new `Transport` object that uses the Serial protocol
    /// and starts its operation.
    ///
    /// TODO(Kaarel): Usage
    fn start(&self) -> Transport {
        // let (tcp_tx, transport_rx) = mpsc::channel();
        // let (transport_tx, tcp_rx) = mpsc::channel();
        // let addr = self.addr;

        // let join_handle = Builder::new()
        //     .name("serial-write".into())
        //     .spawn(move || {
        //         let mut stop = false;
        //         while !stop {
        //             if let Ok(mut stream) = TcpStream::connect(addr) {
        //                 if do_handshake(&mut stream).is_err() {
        //                     tcp_tx.send(Event::Disconnected).unwrap();
        //                     continue;
        //                 }
        //                 tcp_tx.send(Event::Connected).unwrap();

        //                 let thread_tx = tcp_tx.clone();
        //                 let thread_stream = stream.try_clone().unwrap();

        //                 let read_handle = Builder::new()
        //                     .name("sf-read".into())
        //                     .spawn(move || {
        //                         TcpWorker {
        //                             stream: thread_stream,
        //                             tx: thread_tx,
        //                         }
        //                         .start();
        //                     })
        //                     .unwrap();

        //                 {
        //                     let mut worker = ConnectionWorker {
        //                         rx: &tcp_rx,
        //                         stream: stream.try_clone().unwrap(),
        //                     };
        //                     stop = worker.start();
        //                 }

        //                 stream.shutdown(Shutdown::Both).unwrap();
        //                 tcp_tx.send(Event::Disconnected).unwrap();
        //                 read_handle.join().unwrap();

        //                 if !stop {
        //                     thread::sleep(Duration::from_secs(30));
        //                 }
        //             }
        //         }
        //     })
        //     .unwrap();

        // Transport::with_stopper(
        //     transport_tx.clone(),
        //     transport_rx,
        //     Box::new(move || {
        //         if transport_tx.send(Event::Stop).is_err() {
        //             return Err("SFTransport thread already closed!");
        //         }
        //         if join_handle.join().is_err() {
        //             return Err("Unable to join thread!");
        //         }
        //         Ok(())
        //     }),
        // )
        todo!();
    }
}

// impl From<SocketAddr> for SerialBuilder {
//     fn from(addr: SocketAddr) -> Self {
//         SerialBuilder::new(addr)
//     }
// }

// impl TryFrom<String> for SerialBuilder {
//     type Error = std::io::Error;

//     fn try_from(addr: String) -> Result<Self, Self::Error> {
//         let addr = if !addr.contains(':') {
//             format!("{}{}", addr, ":9002")
//         } else {
//             addr
//         };
//         match addr.to_socket_addrs()?.next() {
//             Some(addr) => Ok(SerialBuilder { addr }),
//             None => Err(std::io::Error::new(
//                 std::io::ErrorKind::Other,
//                 "Unable to resolve the address",
//             )),
//         }
//     }
// }

impl SerialWorker {
    fn new(
        tx: Sender<Event>,
        loopback: Sender<Event>,
        waiting_for_ack: Arc<Mutex<HashMap<u8, Bytes>>>,
    ) -> SerialWorker {
        SerialWorker {
            seq_num: 255,
            waiting_for_ack,
            decoder: HdlcCodec::default(),
            tx,
            loopback,
        }
    }

    fn start(&mut self) {
        while let Ok(data) = self.read_from_stream() {
            self.handle_data(data);
        }
    }

    fn handle_data(&mut self, data: Bytes) {
        if !data.is_empty() {
            match data[0] {
                ACKPACKET => self.handle_ack_packet(data),
                ACK => self.handle_ack(data),
                NOACKPACKET => self.handle_no_ack_packet(data),
                other => {
                    // TODO(Kaarel): log message?
                    panic!(format!("Unknown packet type {:02X}!", other));
                }
            };
        }
    }

    fn read_from_stream(&mut self) -> Result<Bytes, std::io::Error> {
        todo!();
    }

    fn decode(&mut self, bytes: &[u8]) -> Vec<Bytes> {
        self.decoder.decode(bytes)
    }

    fn handle_ack_packet(&mut self, data: Bytes) {
        match packets::AckPacket::try_from(data) {
            Ok(packet) => {
                if packet.seq_num > self.seq_num || self.seq_num == 255 {
                    let ack = Ack::from(&packet);
                    self.loopback.send(Event::Data(ack.into())).unwrap();
                    self.seq_num = packet.seq_num;
                    self.tx.send(Event::Data(packet.data)).unwrap();
                } else {
                    // TODO(Kaarel): log message?
                    panic!(format!(
                        "sequence number fault! Recored: {}, incoming: {}",
                        self.seq_num, packet.seq_num
                    ));
                }
            }
            Err(message) => {
                // TODO(Kaarel): log message?
                panic!(message);
            }
        }
    }

    fn handle_ack(&mut self, data: Bytes) {
        match Ack::try_from(data) {
            Ok(packet) => {
                let mut unacked = self.waiting_for_ack.lock().unwrap();
                if unacked.contains_key(&packet.seq_num) {
                    unacked.remove(&packet.seq_num);
                } else {
                    // TODO(Kaarel): log message?
                }
            }
            Err(message) => {
                // TODO(Kaarel): log message?
            }
        }
    }

    fn handle_no_ack_packet(&self, data: Bytes) {
        match NoAckPacket::try_from(data) {
            Ok(packet) => {
                self.tx.send(Event::Data(packet.data)).unwrap();
            }
            Err(message) => {
                // TODO(Kaarel): log message?
            }
        }
    }
}

impl<'a> ConnectionWorker<'a> {
    fn start(&mut self) -> bool {
        loop {
            match self.rx.recv() {
                Ok(v) => match v {
                    Event::Data(d) => match self.write_to_stream(d) {
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::InvalidInput {
                                // TODO(Kaarel): Do something useful!
                            } else {
                                // TODO(Kaarel): Do logging!
                                return false;
                            }
                        }
                        Ok(()) => {}
                    },
                    Event::Stop => {
                        return true;
                    }
                    e => panic!(format!("Unexpected event {:?}", e)),
                },
                Err(e) => panic!(format!("Receive error! {:?}", e)),
            }
        }
    }

    fn write_to_stream(&mut self, data: Bytes) -> Result<(), std::io::Error> {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_ackpacket_decode() {
        let input = vec![
            0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E,
        ];
        let output = vec![
            0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
            0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B,
        ];
        let (tx, _) = mpsc::channel();
        let (serial_worker_tx, _) = mpsc::channel();

        let mut worker = SerialWorker::new(serial_worker_tx, tx, Arc::default());

        let result = worker.decode(&input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], output);
    }
    #[test]
    fn test_ackpacket_handling() {
        // let input = vec![
        //     0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        //     0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B,
        // ];
        // let output = vec![
        //     0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
        //     0x0D, 0x0E, 0x0F,
        // ];
        let input = vec![0x44, 0x00, 0xFF, 0x9D, 0xDF];
        let output = vec![0xFF];
        let ack = vec![0x43, 0x00, 0x9F, 0x58];

        let (tx, connection_worker_rx) = mpsc::channel();
        let (serial_worker_tx, rx) = mpsc::channel();

        let mut worker = SerialWorker::new(serial_worker_tx, tx, Arc::default());

        worker.handle_data(input);

        match rx.recv_timeout(Duration::from_millis(100)).unwrap() {
            Event::Data(result) => assert_eq!(result, output),
            other => panic!("Expected Event::Data, got {:?}", other),
        }

        match connection_worker_rx
            .recv_timeout(Duration::from_millis(100))
            .unwrap()
        {
            Event::Data(result) => assert_eq!(result, ack),
            other => panic!("Expected Event::Data, got {:?}", other),
        }
    }
}
