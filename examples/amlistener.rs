/// Connects to a serial-frowarder port and prints all incoming messages
/// to stdout.
use std::net::ToSocketAddrs;

use chrono::{SecondsFormat, Local};
use clap::{App, Arg};
use regex::Regex;
use serialport::SerialPortSettings;

use moteconnection::dispatcher::am::{AMDispatcherBuilder, AMReceiver};
use moteconnection::ConnectionBuilder;

fn main() {
    let matches = App::new("amlistener")
        .about(concat!(
            "Connects to a serial-frowarder or serial port ",
            "and prints all incoming messages"
        ))
        .arg(
            Arg::with_name("address")
                .help("The address that is ")
                .validator(validate_connection_string)
                .required(true),
        )
        .get_matches();

    stderrlog::new()
        .module("moteconnection")
        .verbosity(log::Level::Info as usize)
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
        .register_dispatcher(&mut dispatcher)
        .start();

    loop {
        if let Ok(message) = receiver.rx.recv() {
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
                let re = Regex::new(r"^([^:]+)(:\d+)?$").unwrap();
                if re.is_match(&name) {
                    let caps = re.captures(&name).unwrap();
                    let name = caps.get(1).unwrap().as_str();
                    let mut settings = SerialPortSettings::default();
                    settings.baud_rate = match caps.get(2) {
                        Some(v) => match String::from(v.as_str()).parse::<u32>() {
                            Ok(b) => b,
                            Err(_) => {
                                return Err(format!("Invalid baud rate {}!", v.as_str()));
                            }
                        },
                        None => 115200,
                    };
                    let ports = serialport::available_ports().unwrap();
                    for port in ports {
                        if port.port_name == name {
                            return Ok(());
                        }
                    }
                }
                Err(format!("The serial port {} was not found!", name))
            }
            protocol => Err(format!("Unknown protocol: {}", protocol)),
        }
    } else {
        Err(format!("Malformed connection string: {}", value))
    }
}
