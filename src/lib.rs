//! The rust implementation of the moteconnection library.
#![deny(missing_docs)]
#![deny(clippy::all)]

mod connection;
pub mod dispatcher;
pub mod transport;

pub use connection::{Connection, ConnectionBuilder};
pub use dispatcher::am::AMDispatcher;
pub use dispatcher::raw::RawDispatcher;
pub use dispatcher::Dispatcher;

/// The way bytes are represented in the crate
pub type Bytes = Vec<u8>;
