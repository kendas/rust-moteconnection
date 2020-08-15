//! The [serial][1] protocol implements the TinyOS serial
//! transport for packets.
//!
//! TODO(Kaarel): More
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/Serial-protocol
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io;
use std::io::ErrorKind;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::Builder;
use std::time::{Duration, Instant};

use regex::Regex;
use serialport::{SerialPort, SerialPortSettings};

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
    name: String,
    settings: SerialPortSettings,
}

struct SerialWorker {
    seq_num_cache: SeqNumCache,
    waiting_for_ack: Arc<Mutex<HashMap<u8, (u8, Bytes)>>>,

    decoder: HdlcCodec,

    tx: Sender<Event>,
    loopback: Sender<Event>,
    stopper: Receiver<()>,
    port: Box<dyn SerialPort>,
}

struct ConnectionWorker<'a> {
    timeout: Duration,

    seq_num: u8,
    waiting_for_ack: Arc<Mutex<HashMap<u8, (u8, Bytes)>>>,

    rx: &'a Receiver<Event>,
    port: Box<dyn SerialPort>,
}

struct SeqNumCache {
    seq_nums: HashMap<u8, Instant>,
    timeout: Duration,
}

impl SerialBuilder {
    /// Creates a new `SerialBuilder`
    ///
    /// TODO(Kaarel): Usage
    pub fn new(name: String, settings: SerialPortSettings) -> SerialBuilder {
        SerialBuilder { name, settings }
    }
}

impl TransportBuilder for SerialBuilder {
    /// Creates a new `Transport` object that uses the Serial protocol
    /// and starts its operation.
    ///
    /// TODO(Kaarel): Usage
    fn start(&self) -> Transport {
        let (tx, connection_rx) = mpsc::channel();
        let (connection_tx, rx) = mpsc::channel();
        let name = self.name.clone();
        let settings = self.settings;
        let loopback_tx = connection_tx.clone();

        let join_handle = Builder::new()
            .name("serial-write".into())
            .spawn(move || {
                let mut stop = false;
                let timeout = Duration::from_millis(100);
                let reconnection_timeout = Duration::from_secs(30);
                while !stop {
                    log::info!("Connecting to {}", name);
                    match serialport::open_with_settings(&name, &settings) {
                        Ok(mut port) => {
                            port.set_timeout(timeout).unwrap();
                            let (read_stop_tx, read_stop_rx) = mpsc::channel();
                            log::info!("Connected.");
                            tx.send(Event::Connected).unwrap();

                            let waiting_for_ack: Arc<Mutex<HashMap<u8, (u8, Bytes)>>> =
                                Arc::default();

                            let mut worker = SerialWorker::new(
                                port.try_clone().unwrap(),
                                tx.clone(),
                                loopback_tx.clone(),
                                read_stop_rx,
                                waiting_for_ack.clone(),
                            );

                            let read_handle = Builder::new()
                                .name("serial-read".into())
                                .spawn(move || {
                                    worker.start();
                                })
                                .unwrap();

                            {
                                let mut worker = ConnectionWorker::new(
                                    port.try_clone().unwrap(),
                                    &rx,
                                    waiting_for_ack.clone(),
                                    timeout,
                                );
                                stop = worker.start();
                            }

                            read_stop_tx.send(()).unwrap();
                            tx.send(Event::Disconnected).unwrap();
                            log::info!("Disconnected.");
                            read_handle.join().unwrap();
                        }
                        Err(e) => {
                            log::warn!("Unable to connect: {:?}", e);
                        }
                    }
                    // Drain the queue and stop if requested.
                    let end = Instant::now() + reconnection_timeout;
                    while !stop {
                        let now = Instant::now();

                        if let Ok(message) = rx.recv_timeout(end - now) {
                            match message {
                                Event::Stop => stop = true,
                                Event::Data(v) => log::debug!(
                                    "Dropping data packet due to being disconnected: {:?}.",
                                    v
                                ),
                                m => {
                                    log::warn!("Unknown message! {:?}", m);
                                    panic!("Unknown message! {:?}", m)
                                }
                            }
                        }
                    }
                }
            })
            .unwrap();

        Transport::with_stopper(
            connection_tx.clone(),
            connection_rx,
            Box::new(move || {
                if connection_tx.send(Event::Stop).is_err() {
                    return Err("SFTransport thread already closed!");
                }
                if join_handle.join().is_err() {
                    return Err("Unable to join thread!");
                }
                Ok(())
            }),
        )
    }
}

impl TryFrom<String> for SerialBuilder {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        let re = Regex::new(r"^([^:]+)(:\d+)?$").unwrap();
        if re.is_match(&name) {
            let caps = re.captures(&name).unwrap();
            let name = caps.get(1).unwrap().as_str();
            let mut settings = SerialPortSettings::default();
            settings.baud_rate = match caps.get(2) {
                Some(v) => match String::from(v.as_str()).parse::<u32>() {
                    Ok(b) => b,
                    Err(_) => {
                        return Err(format!("Invalid baud rate {}!", v.as_str()));
                    }
                },
                None => 115200,
            };
            let ports = serialport::available_ports().unwrap();
            for port in ports {
                if port.port_name == name {
                    return Ok(SerialBuilder::new(String::from(name), settings));
                }
            }
        }
        Err(format!("The serial port {} was not found!", name))
    }
}

impl SerialWorker {
    fn new(
        port: Box<dyn SerialPort>,
        tx: Sender<Event>,
        loopback: Sender<Event>,
        stopper: Receiver<()>,
        waiting_for_ack: Arc<Mutex<HashMap<u8, (u8, Bytes)>>>,
    ) -> SerialWorker {
        SerialWorker {
            port,
            seq_num_cache: SeqNumCache::new(Duration::from_secs(10)),
            waiting_for_ack,
            decoder: HdlcCodec::default(),
            tx,
            loopback,
            stopper,
        }
    }

    fn start(&mut self) {
        while let Ok(packets) = self.read_from_stream() {
            for data in packets {
                self.handle_data(data);
            }
            if let Ok(()) = self.stopper.try_recv() {
                break;
            }
        }
    }

    fn handle_data(&mut self, data: Bytes) {
        if !data.is_empty() {
            match data[0] {
                ACKPACKET => self.handle_ack_packet(data),
                ACK => self.handle_ack(data),
                NOACKPACKET => self.handle_no_ack_packet(data),
                other => {
                    log::debug!("Unknown packet type {:02X}!", other);
                    panic!("Unknown packet type {:02X}!", other);
                }
            };
        }
    }

    fn read_from_stream(&mut self) -> io::Result<Vec<Bytes>> {
        let mut buffer: [u8; 255] = [0; 255];
        match self.port.read(&mut buffer) {
            Ok(length) => Ok(self.decode(&buffer[..length])),
            Err(ref e) if e.kind() == ErrorKind::TimedOut => Ok(vec![]),
            Err(e) => Err(e),
        }
    }

    fn decode(&mut self, bytes: &[u8]) -> Vec<Bytes> {
        self.decoder.decode(bytes)
    }

    fn handle_ack_packet(&mut self, data: Bytes) {
        match packets::AckPacket::try_from(data) {
            Ok(packet) => {
                if let Ok(()) = self.seq_num_cache.insert(packet.seq_num) {
                    let ack = Ack::from(&packet);
                    self.loopback.send(Event::Data(ack.into())).unwrap();
                    self.tx.send(Event::Data(packet.data)).unwrap();
                } else {
                    log::debug!("Sequence number {} on cooldown!", packet.seq_num);
                }
            }
            Err(message) => {
                log::debug!("Malformed AckPacket: {}", message);
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
                    log::debug!(
                        "Received Ack for unknown packet - seq_num: {}",
                        packet.seq_num
                    );
                }
            }
            Err(message) => {
                log::debug!("{}", message);
            }
        }
    }

    fn handle_no_ack_packet(&self, data: Bytes) {
        match NoAckPacket::try_from(data) {
            Ok(packet) => {
                self.tx.send(Event::Data(packet.data)).unwrap();
            }
            Err(message) => {
                log::debug!("{}", message);
            }
        }
    }
}

impl<'a> ConnectionWorker<'a> {
    fn new(
        port: Box<dyn SerialPort>,
        rx: &'a Receiver<Event>,
        waiting_for_ack: Arc<Mutex<HashMap<u8, (u8, Bytes)>>>,
        timeout: Duration,
    ) -> ConnectionWorker {
        ConnectionWorker {
            port,
            timeout,
            seq_num: 0x00,
            waiting_for_ack,
            rx,
        }
    }

    fn start(&mut self) -> bool {
        loop {
            match self.rx.recv_timeout(self.timeout) {
                Ok(v) => match v {
                    Event::Data(d) => self.handle_data(d),
                    Event::Stop => {
                        return true;
                    }
                    e => {
                        log::error!("Unexpected event {:?}", e);
                        panic!("Unexpected event {:?}", e);
                    }
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(e) => {
                    log::error!("Receive error! {:?}", e);
                    panic!("Receive error! {:?}", e);
                }
            }
            self.handle_unacked_data();
        }
    }

    fn handle_data(&mut self, data: Bytes) {
        let packet = AckPacket::new(self.seq_num, data);
        let data: Bytes = packet.into();
        self.write_to_stream(data.clone()).unwrap();
        self.mark_as_waiting_for_ack(data);

        self.bump_seq_num();
    }

    fn handle_unacked_data(&mut self) {
        let mut writes = vec![];
        {
            let mut remove = vec![];
            let mut guard = self.waiting_for_ack.lock().unwrap();
            if !guard.is_empty() {
                for (seq_num, (ref mut retries, data)) in guard.iter_mut() {
                    *retries -= 1;
                    writes.push(data.clone());
                    if retries == &0 {
                        remove.push(*seq_num);
                    }
                }
            }
            for seq_num in remove {
                guard.remove(&seq_num);
            }
        }
        for data in writes {
            self.write_to_stream(data).unwrap();
        }
    }

    fn write_to_stream(&mut self, data: Bytes) -> Result<(), std::io::Error> {
        self.port.write_all(&HdlcCodec::encode(&data))?;
        Ok(())
    }

    fn mark_as_waiting_for_ack(&mut self, data: Bytes) {
        let mut guard = self.waiting_for_ack.lock().unwrap();
        guard.insert(self.seq_num, (3, data));
    }

    fn bump_seq_num(&mut self) {
        if self.seq_num < 0xff {
            self.seq_num += 1;
        } else {
            self.seq_num = 0;
        }
    }
}

impl SeqNumCache {
    fn new(timeout: Duration) -> SeqNumCache {
        SeqNumCache {
            seq_nums: Default::default(),
            timeout,
        }
    }

    fn insert(&mut self, seq_num: u8) -> Result<(), ()> {
        let now = Instant::now();
        match self.seq_nums.get(&seq_num) {
            Some(instant) => {
                if *instant < now - self.timeout {
                    self.seq_nums.insert(seq_num, now);
                    Ok(())
                } else {
                    Err(())
                }
            }
            None => {
                self.seq_nums.insert(seq_num, now);
                Ok(())
            }
        }
    }
}

impl Default for SeqNumCache {
    fn default() -> Self {
        SeqNumCache {
            seq_nums: Default::default(),
            timeout: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::mpsc::TryRecvError;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use utils::TestSerialPort;

    mod utils {
        use std::io;
        use std::io::{Read, Write};
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        use serialport::{
            ClearBuffer, DataBits, FlowControl, Parity, SerialPort, SerialPortSettings, StopBits,
        };

        use super::*;

        #[derive(Clone)]
        pub struct TestSerialPort {
            pub settings: SerialPortSettings,
            pub name: &'static str,
            pub reads: Arc<Mutex<Vec<Bytes>>>,
            pub writes: Arc<Mutex<Vec<Bytes>>>,
        }

        impl TestSerialPort {
            pub fn new() -> Box<TestSerialPort> {
                TestSerialPort::with_settings(SerialPortSettings::default())
            }

            pub fn with_settings(settings: SerialPortSettings) -> Box<TestSerialPort> {
                Box::new(TestSerialPort {
                    settings,
                    name: "testport",
                    reads: Arc::default(),
                    writes: Arc::default(),
                })
            }
        }

        impl SerialPort for TestSerialPort {
            fn name(&self) -> Option<String> {
                Some(String::from(self.name))
            }

            fn settings(&self) -> SerialPortSettings {
                self.settings
            }

            fn baud_rate(&self) -> serialport::Result<u32> {
                Ok(self.settings.baud_rate)
            }

            fn data_bits(&self) -> serialport::Result<DataBits> {
                Ok(self.settings.data_bits)
            }

            fn flow_control(&self) -> serialport::Result<FlowControl> {
                Ok(self.settings.flow_control)
            }

            fn parity(&self) -> serialport::Result<Parity> {
                Ok(self.settings.parity)
            }

            fn stop_bits(&self) -> serialport::Result<StopBits> {
                Ok(self.settings.stop_bits)
            }

            fn timeout(&self) -> Duration {
                Duration::from_secs(0)
            }

            fn set_all(&mut self, settings: &SerialPortSettings) -> serialport::Result<()> {
                self.settings = *settings;
                Ok(())
            }

            fn set_baud_rate(&mut self, baud_rate: u32) -> serialport::Result<()> {
                self.settings.baud_rate = baud_rate;
                Ok(())
            }

            fn set_data_bits(&mut self, data_bits: DataBits) -> serialport::Result<()> {
                self.settings.data_bits = data_bits;
                Ok(())
            }

            fn set_flow_control(&mut self, flow_control: FlowControl) -> serialport::Result<()> {
                self.settings.flow_control = flow_control;
                Ok(())
            }

            fn set_parity(&mut self, parity: Parity) -> serialport::Result<()> {
                self.settings.parity = parity;
                Ok(())
            }

            fn set_stop_bits(&mut self, stop_bits: StopBits) -> serialport::Result<()> {
                self.settings.stop_bits = stop_bits;
                Ok(())
            }

            fn set_timeout(&mut self, timeout: Duration) -> serialport::Result<()> {
                self.settings.timeout = timeout;
                Ok(())
            }

            fn write_request_to_send(&mut self, _level: bool) -> serialport::Result<()> {
                Ok(())
            }

            fn write_data_terminal_ready(&mut self, _level: bool) -> serialport::Result<()> {
                Ok(())
            }

            fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
                Ok(true)
            }

            fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
                Ok(true)
            }

            fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
                Ok(true)
            }

            fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
                Ok(true)
            }

            fn bytes_to_read(&self) -> serialport::Result<u32> {
                Ok(0)
            }

            fn bytes_to_write(&self) -> serialport::Result<u32> {
                Ok(0)
            }

            fn clear(&self, _buffer_to_clear: ClearBuffer) -> serialport::Result<()> {
                Ok(())
            }

            fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
                Ok(Box::new(self.clone()))
            }
        }

        impl Read for TestSerialPort {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let mut reads = self.reads.lock().unwrap();
                if !reads.is_empty() {
                    reads.rotate_left(1);
                    let bytes = reads.pop().unwrap();
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                } else {
                    Ok(0)
                }
            }
        }

        impl Write for TestSerialPort {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                let mut writes = self.writes.lock().unwrap();
                writes.push(buf.to_vec());
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
    }

    #[test]
    fn test_seq_num_cache_insert_success() {
        let mut cache = SeqNumCache::default();
        let sec_num = 0x00;
        let result = cache.insert(sec_num);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_seq_num_cache_insert_fail() {
        let mut cache = SeqNumCache::default();
        let sec_num = 0x00;

        cache.seq_nums.insert(sec_num, Instant::now());
        let result = cache.insert(sec_num);
        assert_eq!(result, Err(()));
    }

    #[test]
    fn test_seq_num_expiration() {
        let mut cache = SeqNumCache::default();
        let sec_num = 0x00;
        let start = Instant::now() - Duration::from_secs(100);

        cache.seq_nums.insert(sec_num, start);

        let result = cache.insert(sec_num);
        assert_eq!(result, Ok(()));
    }

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
        let (_, stopper_rx) = mpsc::channel();

        let port = TestSerialPort::new();

        let mut worker = SerialWorker::new(port, serial_worker_tx, tx, stopper_rx, Arc::default());

        let result = worker.decode(&input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], output);
    }

    #[test]
    fn test_ackpacket_receive() {
        let input = vec![
            0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E,
        ];
        let output = vec![
            0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let ack = Bytes::from(Ack::new(0x00));
        let (tx, worker_rx) = mpsc::channel();
        let (loopback_tx, sender_rx) = mpsc::channel();
        let (stopper_tx, stopper_rx) = mpsc::channel();

        let port = TestSerialPort::new();
        {
            let mut reads = port.reads.lock().unwrap();
            reads.push(input);
        }

        let mut worker = SerialWorker::new(port, tx, loopback_tx, stopper_rx, Arc::default());

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            stopper_tx.send(()).unwrap();
        });
        worker.start();
        handle.join().unwrap();

        match worker_rx.recv_timeout(Duration::from_millis(100)).unwrap() {
            Event::Data(result) => assert_eq!(result, output),
            other => panic!("Expected Event::Data, got {:?}", other),
        }

        match sender_rx.recv_timeout(Duration::from_millis(100)).unwrap() {
            Event::Data(result) => assert_eq!(result, ack),
            other => panic!("Expected Event::Data, got {:?}", other),
        }
    }

    #[test]
    fn test_ackpacket_handling() {
        let input = vec![
            0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
            0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B,
        ];
        let output = vec![
            0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let ack = vec![0x43, 0x00, 0x9F, 0x58];

        let (tx, connection_worker_rx) = mpsc::channel();
        let (serial_worker_tx, rx) = mpsc::channel();
        let (_, stopper_rx) = mpsc::channel();

        let port = TestSerialPort::new();

        let mut worker = SerialWorker::new(port, serial_worker_tx, tx, stopper_rx, Arc::default());

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

    #[test]
    fn test_ack_handling() {
        let input = vec![ACK, 0x00, 0x9f, 0x58];

        let (tx, connection_worker_rx) = mpsc::channel();
        let (serial_worker_tx, rx) = mpsc::channel();
        let (_, stopper_rx) = mpsc::channel();
        let mut waiting_for_ack = HashMap::default();
        waiting_for_ack.insert(0, (3, vec![]));
        let waiting_for_ack = Arc::new(Mutex::new(waiting_for_ack));

        let port = TestSerialPort::new();

        let mut worker = SerialWorker::new(
            port,
            serial_worker_tx,
            tx,
            stopper_rx,
            waiting_for_ack.clone(),
        );

        {
            let guard = waiting_for_ack.lock().unwrap();
            assert_eq!(guard.contains_key(&0x00), true);
        }

        worker.handle_data(input);

        let error = rx.try_recv().unwrap_err();
        assert_eq!(error, TryRecvError::Empty);

        let error = connection_worker_rx.try_recv().unwrap_err();
        assert_eq!(error, TryRecvError::Empty);

        let guard = waiting_for_ack.lock().unwrap();
        assert_eq!(guard.contains_key(&0x00), false);
    }

    #[test]
    fn test_outgoing_packet() {
        let data = vec![
            0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let seq_num = 0x00;
        let packet = AckPacket::new(seq_num, data.clone());

        let (_, connection_worker_rx) = mpsc::channel();
        let waiting_for_ack = Arc::new(Mutex::new(HashMap::default()));

        let port = TestSerialPort::new();

        let mut worker = ConnectionWorker::new(
            port.clone(),
            &connection_worker_rx,
            waiting_for_ack.clone(),
            Duration::from_millis(0),
        );

        {
            let guard = waiting_for_ack.lock().unwrap();
            assert_eq!(guard.is_empty(), true);
        }

        worker.handle_data(data);

        {
            let guard = waiting_for_ack.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard.contains_key(&seq_num), true);
            let (retries, data) = guard.get(&seq_num).unwrap();
            assert_eq!(retries, &3u8);
            assert_eq!(data, &Bytes::from(packet));
        }

        {
            let guard = port.writes.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(
                guard[0],
                vec![
                    0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
                    0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E
                ]
            )
        }
    }

    #[test]
    fn test_ack_count_deincrement() {
        let data = vec![
            0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let seq_num = 0x00;
        let packet = AckPacket::new(seq_num, data);

        let (_, connection_worker_rx) = mpsc::channel();
        let waiting_for_ack = Arc::new(Mutex::new(HashMap::default()));
        {
            let mut guard = waiting_for_ack.lock().unwrap();
            guard.insert(seq_num, (3, packet.clone().into()));
        }

        let port = TestSerialPort::new();

        let mut worker = ConnectionWorker::new(
            port.clone(),
            &connection_worker_rx,
            waiting_for_ack.clone(),
            Duration::from_millis(0),
        );

        worker.handle_unacked_data();

        {
            let guard = waiting_for_ack.lock().unwrap();
            assert_eq!(guard.contains_key(&seq_num), true);
            let (retries, data) = guard.get(&seq_num).unwrap();
            assert_eq!(retries, &2);
            assert_eq!(data, &Bytes::from(packet));
        }

        {
            let guard = port.writes.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(
                guard[0],
                vec![
                    0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
                    0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E
                ]
            )
        }
    }

    #[test]
    fn test_no_ack_resend() {
        let data = vec![
            0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let seq_num = 0x00;
        let packet = AckPacket::new(seq_num, data.clone());

        let (connection_tx, connection_worker_rx) = mpsc::channel();
        let waiting_for_ack = Arc::new(Mutex::new(HashMap::default()));

        let port = TestSerialPort::new();

        let mut worker = ConnectionWorker::new(
            port.clone(),
            &connection_worker_rx,
            waiting_for_ack.clone(),
            Duration::from_millis(0),
        );

        {
            let guard = waiting_for_ack.lock().unwrap();
            assert_eq!(guard.is_empty(), true);
        }

        worker.handle_data(data);

        {
            let guard = waiting_for_ack.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard.contains_key(&seq_num), true);
            let (retries, data) = guard.get(&seq_num).unwrap();
            assert_eq!(retries, &3u8);
            assert_eq!(data, &Bytes::from(packet));
        }

        {
            let guard = port.writes.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(
                guard[0],
                vec![
                    0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
                    0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E
                ]
            );
        }

        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            connection_tx.send(Event::Stop).unwrap();
        });

        let result = worker.start();
        handle.join().unwrap();

        assert_eq!(result, true);

        {
            let guard = port.writes.lock().unwrap();
            assert_eq!(guard.len(), 4);
            for packet in guard.iter() {
                assert_eq!(
                    packet,
                    &vec![
                        0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                        0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E
                    ]
                );
            }
        }
    }

    #[test]
    fn test_pre_connection_stop() {
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let transport = SerialBuilder::new("invalid".to_string(), Default::default()).start();
            transport.stop().unwrap();
            tx.send(()).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        assert_eq!(rx.try_recv().unwrap(), ());
        handle.join().unwrap();
    }
}
