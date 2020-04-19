//! Packet dispatchers handle different packet structures.
//!
//! Dispatchers are packet information schemes that are used
//! for a particular purpose. A dispatcher scheme is identified
//! by a specific ID.
//!
//! The dispatchers provided by this crate by default are:
//! - ActiveMessage (ID=`0x00`)
//! - Raw (ID=any)
pub mod am;
pub mod raw;

/// A dispatcher dispatches incoming packets to interested listeners.
/// 
/// TODO(Kaarel)
pub trait Dispatcher {

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
    // fn receive(&self, data: Vec<u8>);
}
