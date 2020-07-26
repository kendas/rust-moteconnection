/// Connects to a serial-frowarder port and prints all incoming messages
/// to stdout.
///
/// Warning: This program is as simple as possible!. It does not do
/// any fancy argument parsing or anything.
use std::env;

use moteconnection::dispatcher::am::{AMDispatcherBuilder, AMReceiver};
use moteconnection::transport::serialforwarder::SFTransport;
use moteconnection::ConnectionBuilder;

fn main() {
    let addr = env::args().skip(1).next().unwrap();

    let mut dispatcher = AMDispatcherBuilder::new(0x0000);
    let mut receiver = AMReceiver::new();
    dispatcher.register_default_receiver(&mut receiver);
    dispatcher.register_default_snooper(&mut receiver);
    let mut dispatcher = dispatcher.create();

    let _connection = ConnectionBuilder::<SFTransport>::with_connection_string(addr)
        .unwrap()
        .register_dispatcher(&mut dispatcher)
        .start();

    loop {
        if let Ok(message) = receiver.rx.recv() {
            println!(
                "{{{:02X}}}{:04X}->{:04X}[{:02X}]{:>3}: {}",
                message.group,
                message.src,
                message.dest,
                message.id,
                message.payload.len(),
                message
                    .payload
                    .into_iter()
                    .fold(String::new(), |string, byte| format!(
                        "{}{:02X}",
                        string, byte
                    ))
            )
        }
    }
}
