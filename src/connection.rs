//! The connection module contains ...
//!
//! TODO(Kaarel)
//!
use std::collections::HashMap;
use std::convert::{From, TryFrom};

use regex::Regex;

use crate::dispatcher::Dispatcher;
use crate::transport::serialforwarder::SFBuilder;
use crate::transport::Transport;

/// The `Connection` struct manages a persistent connection with a radio module.
///
/// The available transports for the conenction with a radio module that
/// are provided are:
/// - serial
/// - serialforwarder
///
/// TODO(Kaarel): Usage
pub struct Connection {
    #[allow(dead_code)]
    transport: Box<dyn Transport>,
    dispatchers: HashMap<u8, Vec<Box<dyn Dispatcher>>>,
}

impl Connection {
    /// Constructs a new instance of the `Connection` struct.
    ///
    /// TODO(Kaarel): Usage
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Connection {
            transport,
            dispatchers: HashMap::new(),
        }
    }

    // /// Shuts down the connection.
    // /// 
    // /// TODO(Kaarel): Usage
    // pub fn shutdown(&self) -> Result<(), &'static str> {
    //     self.transport.as_ref().stop()?;
    //     Ok(())
    // }

    /// Registers a new dispatcher for a dispatch byte.
    ///
    /// TODO(Kaarel): Usage
    pub fn register_dispatcher(&mut self, dispatcher: Box<dyn Dispatcher>) -> &Self {
        let disp_byte = dispatcher.as_ref().dispatch_byte();
        let dispatchers = match self.dispatchers.get_mut(&disp_byte) {
            Some(v) => v,
            None => {
                let vector = Vec::new();
                self.dispatchers.insert(disp_byte, vector);
                self.dispatchers.get_mut(&disp_byte).unwrap()
            }
        };
        dispatchers.push(dispatcher);
        // dispatcher.as_ref().register_connection(&self);
        self
    }
}

/// Constructs a new instance of the `Connection` struct from a connection string.
///
/// TODO(Kaarel): Usage
impl TryFrom<String> for Connection {
    type Error = String;

    fn try_from(conn_str: String) -> Result<Self, Self::Error> {
        let re = Regex::new(r"^(sf|serial)@([^:]+(:\d+)?)$").unwrap();
        if re.is_match(&conn_str) {
            let caps = re.captures(&conn_str).unwrap();
            match caps.get(1).unwrap().as_str() {
                "sf" => {
                    let builder =
                        match SFBuilder::try_from(String::from(caps.get(2).unwrap().as_str())) {
                            Ok(v) => v,
                            Err(e) => {
                                return Err(format!(
                                    "Error while deconstructing connection string: {}",
                                    e
                                ));
                            }
                        };
                    Ok(Connection::new(Box::new(builder.start())))
                }
                "serial" => Err(String::from("The serial protocol is not implemented yet!")),
                protocol => Err(format!("Unknown protocol: {}", protocol)),
            }
        } else {
            Err(format!("Malformed connection string: {}", conn_str))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Shutdown, TcpListener};
    use std::io::{Read,Write};

    #[test]
    fn test_from_invalid_string() {
        let result = Connection::try_from(String::from("ser-f@no-valid:80"));
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_from_sf_string_no_port() {
        const SERVER_ADDR: &str = "localhost:9002";

        let listener = TcpListener::bind(SERVER_ADDR).unwrap();

        let mut _connection = Connection::try_from(String::from("sf@localhost")).unwrap();

        let data = b"U ";
        let mut server_stream = listener.incoming().next().unwrap().unwrap();
        server_stream.write_all(data).unwrap();

        let mut buffer = [0, 0];
        server_stream.read_exact(&mut buffer).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();
        assert_eq!(&buffer, b"U ");

        // _connection.stop();
    }

    #[test]
    fn test_from_sf_string_explicit_port() {
        const SERVER_ADDR: &str = "localhost:13111";

        let listener = TcpListener::bind(SERVER_ADDR).unwrap();

        let mut _connection = Connection::try_from(String::from("sf@localhost:13111")).unwrap();

        let data = b"U ";
        let mut server_stream = listener.incoming().next().unwrap().unwrap();
        server_stream.write_all(data).unwrap();

        let mut buffer = [0, 0];
        server_stream.read_exact(&mut buffer).unwrap();
        server_stream.shutdown(Shutdown::Both).unwrap();
        assert_eq!(&buffer, b"U ");

        // _connection.stop();
    }
}
