//! The [serial-forwarder][1] protocol implements TCP transport for packets.
//!
//! TODO(Kaarel): More information
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/SerialForwarder-protocol
use std::convert::{From, TryFrom};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use super::Transport;

type Bytes = Vec<u8>;

/// Manages the transportation of moteconnection packets over the TCP
/// serial-formarder protocol.
pub struct SFTransport {
    send: Sender<Bytes>,
    recv: Receiver<Bytes>,
    halt: Sender<()>,
    handle: JoinHandle<()>,
}

impl SFTransport {
    /// Constructs a new `SFTransport` instance.
    pub fn new(
        addr: &SocketAddr,
        // on_connect: Option<Box<dyn Fn()>>,
        // on_disconnect: Option<Box<dyn Fn()>>,
    ) -> Self {
        let (outgoing_send, outgoing_recv) = mpsc::channel();
        let (incoming_send, incoming_recv) = mpsc::channel();
        let (control_send, control_recv) = mpsc::channel();
        let addr = *addr;

        let handle = thread::spawn(move || {
            if let Ok(mut stream) = TcpStream::connect(addr) {
                // if let Some(func) = on_connect {
                //     func();
                // }
                do_handshake(&mut stream).unwrap();
                while let Err(TryRecvError::Empty) = control_recv.try_recv() {
                    if let Ok(Some(bytes)) = read_from_stream(&mut stream) {
                        incoming_send.send(bytes).unwrap();
                    }
                    if let Ok(bytes) = outgoing_recv.try_recv() {
                        write_to_stream(&mut stream, bytes).unwrap();
                    }
                }
            }
            // if let Some(func) = on_disconnect {
            //     func();
            // }
        });
        SFTransport {
            send: outgoing_send,
            recv: incoming_recv,
            halt: control_send,
            handle,
        }
    }
}

impl Transport for SFTransport {
    fn send_packet(&self, data: Bytes) -> Result<(), &'static str> {
        if self.send.send(data).is_err() {
            Err("Data receiver closed!")
        } else {
            Ok(())
        }
    }

    fn receive_packet(&self) -> Result<Bytes, TryRecvError> {
        Ok(self.recv.try_recv()?)
    }

    fn stop(self) -> Result<(), &'static str> {
        if self.halt.send(()).is_err() {
            return Err("Control receiver closed!");
        };
        if self.handle.join().is_err() {
            return Err("Unable to join thread!");
        }
        Ok(())
    }
}

/// A builder object for `SFTransport`
pub struct SFBuilder {
    addr: SocketAddr,
    connect_callback: Option<Box<dyn Fn()>>,
    disconnect_callback: Option<Box<dyn Fn()>>,
}

impl SFBuilder {
    /// Creates a new `SFBuilder`
    pub fn new(addr: SocketAddr) -> Self {
        SFBuilder {
            addr,
            connect_callback: None,
            disconnect_callback: None,
        }
    }

    /// Registers a callback function for signaling a successful connection.
    pub fn on_connect(&mut self, callback: Box<dyn Fn()>) {
        self.connect_callback = Some(callback);
    }

    /// Registers a callback function for signaling a disconnect ecent.
    pub fn on_disconnect(&mut self, callback: Box<dyn Fn()>) {
        self.disconnect_callback = Some(callback);
    }

    /// Creates an `SFTransport` object and starts its operation
    pub fn start(&self) -> SFTransport {
        SFTransport::new(&self.addr)
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
        match addr.to_socket_addrs()?.next() {
            Some(addr) => Ok(SFBuilder {
                addr,
                connect_callback: None,
                disconnect_callback: None,
            }),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unable to resolve the address",
            )),
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

fn read_from_stream(stream: &mut TcpStream) -> Result<Option<Bytes>, std::io::Error> {
    use std::io::ErrorKind;

    stream.set_read_timeout(Some(Duration::from_nanos(1)))?;
    let mut packet_length: [u8; 1] = [0];
    if let Err(err) = stream.read_exact(&mut packet_length) {
        match err.kind() {
            ErrorKind::WouldBlock => return Ok(None),
            ErrorKind::TimedOut => return Ok(None),
            _ => return Err(err),
        }
    }

    // We know a packet is coming. Disable read timeout.
    stream.set_read_timeout(None)?;
    let mut packet_buf = vec![0; packet_length[0] as usize];
    stream.read_exact(&mut packet_buf[..])?;

    Ok(Some(packet_buf))
}

fn write_to_stream(stream: &mut TcpStream, data: Bytes) -> Result<(), std::io::Error> {
    use std::io::{Error, ErrorKind};
    if let Ok(length) = u8::try_from(data.len()) {
        stream.write_all(&[length])?;
        stream.write_all(&data[..])?;
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::InvalidData,
            "Packet size must not exceed 255!",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Shutdown, TcpListener, TcpStream};

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
        let mut client_stream = TcpStream::connect(SERVER_ADDR).unwrap();
        let mut server_stream = listener.incoming().next().unwrap().unwrap();

        let data = vec![0, 1, 2, 3, 4, 5];
        let mut expected_result = vec![u8::try_from(data.len()).unwrap()];
        expected_result.append(&mut data.clone());
        let expected_result = expected_result;

        let result = write_to_stream(&mut client_stream, data);
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
        let mut client_stream = TcpStream::connect(SERVER_ADDR).unwrap();
        let mut server_stream = listener.incoming().next().unwrap().unwrap();

        let expected_result = vec![0, 1, 2, 3, 4, 5];
        let mut data = vec![u8::try_from(expected_result.len()).unwrap()];
        data.append(&mut expected_result.clone());
        let data = data;
        println!("{:?}", data);

        server_stream.write_all(&data[..]).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();

        let result = read_from_stream(&mut client_stream).unwrap();
        client_stream.shutdown(Shutdown::Both).unwrap();

        assert_eq!(result, Some(expected_result));
    }

    #[test]
    fn test_stream_read_timeout() {
        const SERVER_ADDR: &str = "localhost:7881";
        let listener = TcpListener::bind(SERVER_ADDR).unwrap();
        let mut client_stream = TcpStream::connect(SERVER_ADDR).unwrap();
        let server_stream = listener.incoming().next().unwrap().unwrap();

        let result = read_from_stream(&mut client_stream).unwrap();
        client_stream.shutdown(Shutdown::Both).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();

        assert!(result.is_none());
    }
}
