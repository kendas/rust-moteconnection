/// Connects to a serial-forwarder port and prints all incoming messages
/// to stdout.
use std::net::ToSocketAddrs;
use std::time::Duration;

use chrono::{Local, SecondsFormat};
use clap::{App, Arg};
use regex::Regex;

use moteconnection::{ConnectionBuilder, Event};
use moteconnection::dispatcher::am::{AMDispatcherBuilder, AMReceiver};

fn main() {
    let matches = App::new("amlistener")
        .about(concat!(
            "Connects to a serial-forwarder or serial port ",
            "and prints all incoming messages"
        ))
        .arg(
            Arg::with_name("address")
                .validator(validate_connection_string)
                .required(true)
                .help("The address that is "),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let verbosity = match matches.occurrences_of("v") {
        0 => log::Level::Error,
        1 => log::Level::Warn,
        2 => log::Level::Info,
        3 | _ => log::Level::Debug,
    };
    stderrlog::new()
        .module("moteconnection")
        .verbosity(verbosity as usize)
        .timestamp(stderrlog::Timestamp::Microsecond)
        .init()
        .unwrap();

    let addr = matches.value_of("address").unwrap().to_string();

    let mut receiver = AMReceiver::new();
    let mut dispatcher = AMDispatcherBuilder::new(0x0000)
        .group(None)
        .register_default_receiver(&mut receiver)
        .register_default_snooper(&mut receiver)
        .create();

    let _connection = ConnectionBuilder::with_connection_string(addr)
        .unwrap()
        .reconnect_timeout(Duration::from_secs(10))
        .register_dispatcher(&mut dispatcher)
        .start();

    loop {
        if let Ok(event) = receiver.rx.recv() {
            match event {
                Event::Data(message) => {
                    let metadata = if message.metadata.len() == 2 {
                        format!(
                            "{:02X}:{}",
                            u8::from_be_bytes([message.metadata[0]]),
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
                        Local::now().to_rfc3339_opts(SecondsFormat::Micros, true),
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
                Event::Connected => println!(
                    "{} Connected.",
                    Local::now().to_rfc3339_opts(SecondsFormat::Micros, true)
                ),
                Event::Disconnected => println!(
                    "{} Disconnected.",
                    Local::now().to_rfc3339_opts(SecondsFormat::Micros, true)
                ),
                o => println!("Unknown event {:?}", o),
            }
        }
    }
}

fn validate_connection_string(value: String) -> Result<(), String> {
    let re = Regex::new(r"^(sf|serial)@([^:]+(:\d+)?)$").unwrap();
    if re.is_match(&value) {
        let caps = re.captures(&value).unwrap();
        match caps.get(1).unwrap().as_str() {
            "sf" => {
                let mut addr: String = caps.get(2).unwrap().as_str().into();
                if !addr.contains(":") {
                    addr = format!("{}:9002", addr);
                }
                match addr.to_socket_addrs() {
                    Err(_) => Err(format!("{} is not a valid network address!", addr)),
                    _ => Ok(()),
                }
            }
            "serial" => {
                let name = caps.get(2).unwrap().as_str();
                let re = Regex::new(r"^([^:]+)(?::(\d+))?$").unwrap();
                if re.is_match(&name) {
                    let caps = re.captures(&name).unwrap();
                    let name = caps.get(1).unwrap().as_str();
                    if let Some(v) = caps.get(2) {
                        if String::from(v.as_str()).parse::<u32>().is_err() {
                            return Err(format!("Invalid baud rate {}", v.as_str()));
                        }
                    }
                    let port_exists = serialport::available_ports()
                        .unwrap()
                        .iter()
                        .find(|port| port.port_name == name)
                        .is_some();
                    if !port_exists {
                        return Err(format!("The serial port {} was not found", name));
                    }
                    Ok(())
                } else {
                    Err(format!(
                        "The serial port information {} uses unknown format",
                        name
                    ))
                }
            }
            protocol => Err(format!("Unknown protocol: {}", protocol)),
        }
    } else {
        Err(format!("Malformed connection string: {}", value))
    }
}
