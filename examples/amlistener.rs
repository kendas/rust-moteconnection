/// Connects to a serial-frowarder port and prints all incoming messages
/// to stdout.
///
/// Warning: This program is as simple as possible!. It does not do
/// any fancy argument parsing or anything.
use std::env;

use chrono::{SecondsFormat, Utc};

use moteconnection::dispatcher::am::{AMDispatcherBuilder, AMReceiver};
use moteconnection::ConnectionBuilder;

fn main() {
    let addr = env::args().skip(1).next().unwrap();

    let mut receiver = AMReceiver::new();
    let mut dispatcher = AMDispatcherBuilder::new(0x0000);
    dispatcher.group(None);
    dispatcher.register_default_receiver(&mut receiver);
    dispatcher.register_default_snooper(&mut receiver);
    let mut dispatcher = dispatcher.create();

    let _connection = ConnectionBuilder::with_connection_string(addr)
        .unwrap()
        .register_dispatcher(&mut dispatcher)
        .start();

    loop {
        if let Ok(message) = receiver.rx.recv() {
            let metadata = if message.metadata.len() == 2 {
                format!(
                    "{:02X}:{}",
                    i8::from_be_bytes([message.metadata[0]]),
                    i8::from_be_bytes([message.metadata[1]])
                )
            } else {
                message
                    .metadata
                    .into_iter()
                    .fold(String::new(), |string, byte| {
                        format!("{}{:02X}", string, byte)
                    })
            };
            println!(
                "{} {{{:02X}}}{:04X}->{:04X}[{:02X}]{:>3}: {} {}",
                Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true),
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
                    )),
                metadata,
            )
        }
    }
}