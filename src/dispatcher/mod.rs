//! Packet dispatchers handle different packet structures.
//!
//! Dispatchers are packet information schemes that are used
//! for a particular purpose. A dispatcher scheme is identified
//! by a specific ID.
//!
//! The dispatchers provided by this crate by default are:
//! - ActiveMessage (ID=`0x00`)
//! - Raw (ID=any)
use std::convert::TryFrom;

pub mod am;
pub mod raw;

/// A dispatcher dispatches incoming packets to interested listeners.
///
/// TODO(Kaarel)
pub trait Dispatcher {
    /// Returns the dispatch byte of the dispatcher
    fn dispatch_byte(&self) -> u8;

    // /// Attaches a sender.
    // fn attach(&mut self, sender: Fn(&dyn Vec<u8>));
    // // def attach(self, sender):
    // //     self._sender = sender

    // /// Detatches the sender.
    // fn detatch(&mut self);
    // // def detach(self):
    // //     self._sender = None

    // /// Sends data to the radio module.
    // fn send(&self, data: Vec<u8>);

    // /// Received data from the radio module.
    // fn receive(&self, data: DispatchPacket);
}

/// A packet with the dispatch byte and a payload.
#[derive(Debug)]
pub struct DispatchPacket {
    /// The dispatcher ID of the payload
    pub dispatch: u8,
    /// The payload to be interpreted by the dispatcher
    pub payload: Vec<u8>,
}

impl TryFrom<Vec<u8>> for DispatchPacket {
    type Error = &'static str;
    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        match data.len() {
            0 => Err("Data length is 0!"),
            _ => Ok(DispatchPacket {
                dispatch: data[0],
                payload: data[1..].to_vec(),
            }),
        }
    }
}

impl Into<Vec<u8>> for DispatchPacket {
    fn into(self) -> Vec<u8> {
        let mut result = Vec::with_capacity(1 + self.payload.len());
        result.extend([self.dispatch].iter().chain(self.payload.iter()));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_dispatchpacket_normal() {
        let data = vec![1, 2, 3, 4];
        let packet = DispatchPacket::try_from(data).unwrap();
        assert_eq!(packet.dispatch, 1);
        assert_eq!(packet.payload, vec![2, 3, 4]);
    }

    #[test]
    fn test_bytes_to_dispatchpacket_no_data() {
        let data = vec![1];
        let packet = DispatchPacket::try_from(data).unwrap();
        assert_eq!(packet.dispatch, 1);
        assert_eq!(packet.payload, vec![]);
    }

    #[test]
    fn test_bytes_to_dispatchpacket_zero_length() {
        let data = vec![];
        let error = DispatchPacket::try_from(data).unwrap_err();
        assert_eq!(error, "Data length is 0!");
    }

    #[test]
    fn test_dispatchpacket_into_bytes() {
        let packet = DispatchPacket { dispatch: 5, payload: vec![1, 2, 3]};
        let bytes: Vec<u8> = packet.into();
        assert_eq!(bytes, vec![5, 1, 2, 3]);
    }
}
