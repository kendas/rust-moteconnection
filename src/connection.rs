//! The connection module contains ...
//!
//! TODO(Kaarel)
//!
use std::collections::HashMap;
use std::convert::{From, TryFrom};
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};

use regex::Regex;

use crate::dispatcher::{Dispatcher, DispatcherHandle};
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
    transport: Transport,
    dispatchers: HashMap<u8, Arc<Mutex<DispatcherHandle>>>,
}

/// A builder for the connection struct
///
/// TODO(Kaarel): Usage
pub struct ConnectionBuilder {
    dispatchers: HashMap<u8, Arc<Mutex<DispatcherHandle>>>,
    connection_string: String,
}

impl Connection {
    /// Constructs a new instance of the `Connection` struct.
    ///
    /// TODO(Kaarel): Usage
    pub fn new(
        connection_string: &str,
        dispatchers: &HashMap<u8, Dispatcher>,
    ) -> Result<Connection, String> {
        let handles = HashMap::from_iter(dispatchers.iter().map(|(dispatch_byte, dispatcher)| {
            (dispatch_byte.to_owned(), dispatcher.get_handle())
        }));

        Connection::with_handles(connection_string, handles)
    }

    fn with_handles(
        connection_string: &str,
        handles: HashMap<u8, Arc<Mutex<DispatcherHandle>>>,
    ) -> Result<Connection, String> {
        let re = Regex::new(r"^(sf|serial)@([^:]+(:\d+)?)$").unwrap();
        if re.is_match(connection_string) {
            let caps = re.captures(connection_string).unwrap();
            let builder = match caps.get(1).unwrap().as_str() {
                "sf" => match SFBuilder::try_from(String::from(caps.get(2).unwrap().as_str())) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!(
                            "Error while deconstructing connection string: {}",
                            e
                        ));
                    }
                },
                "serial" => {
                    return Err(String::from("The serial protocol is not implemented yet!"));
                }
                protocol => {
                    return Err(format!("Unknown protocol: {}", protocol));
                }
            };

            Ok(Connection {
                transport: builder.start(),
                dispatchers: handles,
            })
        } else {
            Err(format!(
                "Malformed connection string: {}",
                connection_string
            ))
        }
    }

    /// Shuts down the connection.
    ///
    /// TODO(Kaarel): Usage
    pub fn stop(self) -> Result<(), &'static str> {
        self.transport.stop()?;
        Ok(())
    }
}

impl ConnectionBuilder {
    /// Creates a new ConnectionBuilder
    pub fn new(connection_string: String) -> ConnectionBuilder {
        ConnectionBuilder {
            dispatchers: HashMap::new(),
            connection_string,
        }
    }

    /// Registers a new dispatcher for a dispatch byte.
    ///
    /// # Example
    ///
    /// ```rust
    /// use moteconnection::{AMDispatcher, ConnectionBuilder, DispatcherBuilder};
    ///
    /// let dispatcher_builder = AMDispatcher::new(0x0201);
    /// let dispatcher = dispatcher_builder.create();
    /// let connection_builder = ConnectionBuilder::new("sf@localhost".into())
    ///     .register_dispatcher(&dispatcher);
    /// ```
    pub fn register_dispatcher(&mut self, dispatcher: &Dispatcher) -> &Self {
        self.dispatchers
            .insert(dispatcher.dispatch_byte(), dispatcher.get_handle());
        self
    }

    /// Establishes a new connection and returns the handler.
    ///
    /// # Example
    ///
    /// ```rust
    /// ```
    pub fn start(&self) -> Result<Connection, String> {
        Ok(Connection::with_handles(
            &self.connection_string,
            self.dispatchers.clone(),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener};

    #[test]
    fn test_from_invalid_string() {
        let result = Connection::new("ser-f@no-valid:80", &HashMap::new());
        assert_eq!(result.is_err(), true);
    }

    #[test]
    fn test_from_sf_string_no_port() {
        const SERVER_ADDR: &str = "localhost:9002";

        let listener = TcpListener::bind(SERVER_ADDR).unwrap();

        let mut _connection = Connection::new("sf@localhost", &HashMap::new()).unwrap();

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

        let mut _connection = Connection::new("sf@localhost:13111", &HashMap::new()).unwrap();

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
