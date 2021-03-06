//! The [serial-forwarder][1] protocol implements TCP transport for packets.
//!
//! TODO(Kaarel): More information
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/SerialForwarder-protocol
use std::convert::{From, TryFrom};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::Builder;
use std::time::{Duration, Instant};

use super::{Transport, TransportBuilder};
use crate::{Bytes, Event};

/// A builder object for the serial-forwarder `Transport`
pub struct SFBuilder {
    addr: SocketAddr,
    reconnect_timeout: Duration,
}

struct TcpWorker {
    stream: TcpStream,
    tx: Sender<Event<Bytes>>,
    loopback_tx: Sender<Event<Bytes>>,
}

struct ConnectionWorker<'a> {
    rx: &'a Receiver<Event<Bytes>>,
    stream: TcpStream,
}

impl SFBuilder {
    /// Creates a new `SFBuilder`
    ///
    /// TODO(Kaarel): Usage
    pub fn new(addr: SocketAddr) -> Self {
        SFBuilder {
            addr,
            reconnect_timeout: Duration::from_secs(30),
        }
    }
}

impl TransportBuilder for SFBuilder {
    fn start(&self) -> Transport {
        let (tcp_tx, transport_rx) = mpsc::channel();
        let (transport_tx, tcp_rx) = mpsc::channel();
        let loopback_tx = transport_tx.clone();
        let addr = self.addr;
        let reconnection_timeout = self.reconnect_timeout;

        let join_handle = Builder::new()
            .name("sf-write".into())
            .spawn(move || {
                let mut stop = false;
                while !stop {
                    log::info!("Connecting to {}", addr);
                    if let Ok(mut stream) = TcpStream::connect(addr) {
                        log::info!("Connected.");
                        let mut read_handle = None;
                        if do_handshake(&mut stream).is_err() {
                            tcp_tx.send(Event::Disconnected).unwrap();
                            log::debug!("Handshake failed.");
                        } else {
                            log::debug!("Handshake successful.");
                            tcp_tx.send(Event::Connected).unwrap();

                            let thread_tx = tcp_tx.clone();
                            let thread_stream = stream.try_clone().unwrap();
                            let loopback_tx = loopback_tx.clone();

                            read_handle = Some(
                                Builder::new()
                                    .name("sf-read".into())
                                    .spawn(move || {
                                        TcpWorker {
                                            stream: thread_stream,
                                            tx: thread_tx,
                                            loopback_tx,
                                        }
                                        .start();
                                    })
                                    .unwrap(),
                            );

                            {
                                let mut worker = ConnectionWorker {
                                    rx: &tcp_rx,
                                    stream: stream.try_clone().unwrap(),
                                };
                                stop = worker.start();
                            }
                        }

                        stream.shutdown(Shutdown::Both).unwrap();
                        tcp_tx.send(Event::Disconnected).unwrap();
                        log::info!("Disconnected.");
                        if let Some(handle) = read_handle {
                            handle.join().unwrap();
                        }
                    }
                    // Drain the queue and stop if requested.
                    let end = Instant::now() + reconnection_timeout;
                    while !stop {
                        let now = Instant::now();
                        if now > end {
                            break;
                        }

                        if let Ok(message) = tcp_rx.recv_timeout(end - now) {
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
            transport_tx.clone(),
            transport_rx,
            Box::new(move || {
                if transport_tx.send(Event::Stop).is_err() {
                    return Err("SFTransport thread already closed!");
                }
                if join_handle.join().is_err() {
                    return Err("Unable to join thread!");
                }
                Ok(())
            }),
        )
    }

    fn set_reconnect_timeout(&mut self, timeout: Duration) {
        self.reconnect_timeout = timeout;
    }
}

impl From<SocketAddr> for SFBuilder {
    fn from(addr: SocketAddr) -> Self {
        SFBuilder::new(addr)
    }
}

impl TryFrom<String> for SFBuilder {
    type Error = std::io::Error;

    fn try_from(addr: String) -> Result<Self, Self::Error> {
        let addr = if !addr.contains(':') {
            format!("{}{}", addr, ":9002")
        } else {
            addr
        };
        match addr.to_socket_addrs()?.next() {
            Some(addr) => Ok(SFBuilder::new(addr)),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unable to resolve the address",
            )),
        }
    }
}

impl TcpWorker {
    fn start(&mut self) {
        loop {
            match self.read_from_stream() {
                Ok(data) => {
                    if !data.is_empty() {
                        self.tx.send(Event::Data(data)).unwrap();
                    }
                }
                Err(e) => {
                    log::info!("Error while reading from the TCP stream: {}", e);
                    self.loopback_tx.send(Event::Disconnected).unwrap();
                    break;
                }
            }
        }
    }

    fn read_from_stream(&mut self) -> Result<Bytes, std::io::Error> {
        let mut packet_length: [u8; 1] = [0];
        self.stream.read_exact(&mut packet_length)?;
        // We know a packet is coming. Disable read timeout.
        self.stream.set_read_timeout(None)?;
        let mut packet_buf = vec![0; packet_length[0] as usize];
        self.stream.read_exact(&mut packet_buf[..])?;
        Ok(packet_buf)
    }
}

impl<'a> ConnectionWorker<'a> {
    fn start(&mut self) -> bool {
        loop {
            match self.rx.recv() {
                Ok(v) => match v {
                    Event::Data(d) => match self.write_to_stream(d) {
                        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {}
                        Err(e) => {
                            log::warn!(
                                "Received unexpected error while writing to the stream: {:?}",
                                e
                            );
                            return false;
                        }
                        Ok(()) => {}
                    },
                    Event::Disconnected => {
                        return false;
                    }
                    Event::Stop => {
                        return true;
                    }
                    e => {
                        log::debug!("Unexpected event {:?}", e);
                        panic!("Unexpected event {:?}", e);
                    }
                },
                Err(e) => {
                    log::debug!("Receive error! {:?}", e);
                    panic!("Receive error! {:?}", e);
                }
            }
        }
    }

    fn write_to_stream(&mut self, data: Bytes) -> Result<(), std::io::Error> {
        use std::io::{Error, ErrorKind};
        if let Ok(length) = u8::try_from(data.len()) {
            self.stream.write_all(&[length])?;
            self.stream.write_all(&data[..])?;
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "Packet size must not exceed 255!",
            ))
        }
    }
}

fn do_handshake(stream: &mut TcpStream) -> Result<(), &'static str> {
    const PROTOCOL: u8 = 0x55;
    const VERSION: u8 = 0x20;

    let mut buf = [0; 2];
    match stream.read(&mut buf) {
        Ok(2) => {
            let buf = buf;
            if buf[0] != PROTOCOL {
                Err("Incorrect protocol byte!")
            } else if buf[1] != VERSION {
                Err("Incorrect protocol version!")
            } else {
                if stream.write_all(&[PROTOCOL, VERSION]).is_err() {
                    return Err("Handshake response failed to send!");
                }
                Ok(())
            }
        }
        Ok(_) => Err("Handshake length was incorrect!"),
        Err(_) => Err("Error while establishing connection!"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn test_default_port() {
        let addr = String::from("localhost");
        let builder = SFBuilder::try_from(addr).unwrap();
        assert_eq!(builder.addr.port(), 9002)
    }

    #[test]
    fn test_explicit_port() {
        let addr = String::from("localhost:12334");
        let builder = SFBuilder::try_from(addr).unwrap();
        assert_eq!(builder.addr.port(), 12334)
    }

    #[test]
    fn test_handshake() {
        const SERVER_ADDR: &str = "localhost:7878";
        let listener = TcpListener::bind(SERVER_ADDR).unwrap();

        let mut client_stream = TcpStream::connect(SERVER_ADDR).unwrap();

        let data = b"U ";
        let mut server_stream = listener.incoming().next().unwrap().unwrap();
        server_stream.write_all(data).unwrap();

        assert_eq!(do_handshake(&mut client_stream), Ok(()));
        client_stream.shutdown(Shutdown::Both).unwrap();

        let mut response = Vec::new();
        server_stream.read_to_end(&mut response).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();

        assert_eq!(&response[..], data);
    }

    #[test]
    fn test_stream_write() {
        const SERVER_ADDR: &str = "localhost:7879";
        let listener = TcpListener::bind(SERVER_ADDR).unwrap();
        let client_stream = TcpStream::connect(SERVER_ADDR).unwrap();
        let mut server_stream = listener.incoming().next().unwrap().unwrap();

        let data = vec![0, 1, 2, 3, 4, 5];
        let mut expected_result = vec![u8::try_from(data.len()).unwrap()];
        expected_result.append(&mut data.clone());
        let expected_result = expected_result;

        let (_, rx) = mpsc::channel();
        let mut worker = ConnectionWorker {
            rx: &rx,
            stream: client_stream.try_clone().unwrap(),
        };

        let result = worker.write_to_stream(data);
        client_stream.shutdown(Shutdown::Both).unwrap();

        assert!(result.is_ok());

        let mut response = Vec::new();
        server_stream.read_to_end(&mut response).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();

        assert_eq!(response, expected_result);
    }

    #[test]
    fn test_stream_read_success() {
        const SERVER_ADDR: &str = "localhost:7880";
        let listener = TcpListener::bind(SERVER_ADDR).unwrap();
        let client_stream = TcpStream::connect(SERVER_ADDR).unwrap();
        let mut server_stream = listener.incoming().next().unwrap().unwrap();

        let expected_result = vec![0, 1, 2, 3, 4, 5];
        let mut data = vec![u8::try_from(expected_result.len()).unwrap()];
        data.append(&mut expected_result.clone());
        let data = data;

        let (tx, _) = mpsc::channel();
        let mut worker = TcpWorker {
            stream: client_stream.try_clone().unwrap(),
            tx: tx.clone(),
            loopback_tx: tx,
        };

        server_stream.write_all(&data[..]).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();

        let result = worker.read_from_stream().unwrap();
        client_stream.shutdown(Shutdown::Both).unwrap();

        assert_eq!(result, expected_result);
    }

    #[test]
    fn test_stop() {
        const SERVER_ADDR: &str = "localhost:7881";
        let listener = TcpListener::bind(SERVER_ADDR).unwrap();
        let transport = SFBuilder::try_from(String::from(SERVER_ADDR))
            .unwrap()
            .start();

        let data = b"U ";
        let mut server_stream = listener.incoming().next().unwrap().unwrap();
        server_stream.write_all(data).unwrap();
        let mut handshake = [0, 0];
        server_stream.read_exact(&mut handshake).unwrap();
        assert_eq!(handshake, [0x55, 0x20]);

        (transport.stopper)().unwrap();

        let mut read_buf = vec![];
        match server_stream.read_to_end(&mut read_buf) {
            Ok(0) => {}
            v => panic!(format!("Expected a closed stream, got {:?}", v)),
        }
        assert_eq!(read_buf, vec![]);
    }

    #[test]
    fn test_pre_connection_stop() {
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let transport = SFBuilder::try_from(String::from("127.255.255.255:12345"))
                .unwrap()
                .start();
            transport.stop().unwrap();
            tx.send(()).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        assert_eq!(rx.try_recv().unwrap(), ());
        handle.join().unwrap();
    }
}
