//! Transport layer implementations enable receiving and sending packets
//! using different communication protocols.
//!
//! A transport layer is responsible the low level communication between
//! the radio module to the `moteconnection` library.
pub mod serial;
pub mod serialforwarder;
