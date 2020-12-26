//! The rust implementation of the moteconnection library.
//! 
//! # Example
//! 
//! ```rust
//! use moteconnection::{ConnectionBuilder, RawDispatcher};
//!
//! let mut dispatcher = RawDispatcher::new(0x01);
//! let connection = ConnectionBuilder::with_connection_string("sf@localhost".to_string())
//!     .unwrap()
//!     .register_dispatcher(&mut dispatcher)
//!     .start();
//! ```
#![deny(missing_docs)]
#![deny(clippy::all)]

mod connection;
pub mod dispatcher;
pub mod transport;
mod tests;

pub use connection::{Connection, ConnectionBuilder};
pub use dispatcher::am::AMDispatcher;
pub use dispatcher::raw::RawDispatcher;
pub use dispatcher::Dispatcher;

/// The way bytes are represented in the crate
pub type Bytes = Vec<u8>;

/// The types of messages sent to or from the various components.
#[derive(Debug)]
pub enum Event<T> {
    /// Signals that the underlying connection has disconnected
    Disconnected,
    /// Signals that the underlying connection has been established
    Connected,
    /// Contains any data being sent.
    Data(T),
    /// Signals the stopping of the connection.
    Stop,
    /// Signals that there is an error.
    Error,
}

