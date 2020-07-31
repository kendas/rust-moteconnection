//! Provides the packet types used in the serial protocol.
use std::convert::{TryFrom, TryInto};

use crc::crc16;

use super::{ACK, ACKPACKET, NOACKPACKET};
use crate::Bytes;

/// The acknowledged packet
pub struct AckPacket {
    /// The protocol byte
    pub protocol: u8,
    /// The sequence number byte
    pub seq_num: u8,
    /// The data
    pub data: Bytes,
    /// The CRC checksum (little-endian)
    pub checksum: u16,
}

/// The acknowledgement packet
pub struct Ack {
    /// The protocol byte
    pub protocol: u8,
    /// The sequence number byte
    pub seq_num: u8,
    /// The CRC checksum (little-endian)
    pub checksum: u16,
}

/// The unacknowledged packet
pub struct NoAckPacket {
    /// The protocol byte
    pub protocol: u8,
    /// The data
    pub data: Bytes,
    /// The CRC checksum (little-endian)
    pub checksum: u16,
}

impl AckPacket {
    /// Creates a new instance of `AckPacket`.
    pub fn new(seq_num: u8, data: Bytes) -> AckPacket {
        let mut payload = Vec::with_capacity(2 + data.len());
        payload.push(ACKPACKET);
        payload.push(seq_num);
        payload.extend(data.clone());
        let checksum = crc16::checksum_x25(&payload);
        AckPacket {
            protocol: ACKPACKET,
            seq_num,
            data,
            checksum,
        }
    }
}

impl TryFrom<Bytes> for AckPacket {
    type Error = String;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        if value.len() >= 5 {
            if value[0] == ACKPACKET {
                let checksum =
                    u16::from_le_bytes(value.split_off(value.len() - 2)[..].try_into().unwrap());

                if crc16::checksum_x25(&value) == checksum {
                    let mut protocol = value;
                    let mut seq_num = protocol.split_off(1);
                    let protocol = protocol[0];
                    let data = seq_num.split_off(1);
                    let seq_num = seq_num[0];

                    Ok(AckPacket {
                        protocol,
                        seq_num,
                        data,
                        checksum,
                    })
                } else {
                    Err("Invalid checksum!".into())
                }
            } else {
                Err(format!(
                    "Invalid protocol byte {} for AckPacket!",
                    value.len()
                ))
            }
        } else {
            Err(format!("Invalid length {} for AckPacket!", value.len()))
        }
    }
}

impl From<AckPacket> for Bytes {
    fn from(value: AckPacket) -> Self {
        let mut result = Vec::with_capacity(4 + value.data.len());
        result.extend(
            [value.protocol, value.seq_num]
                .iter()
                .chain(value.data.iter())
                .chain(value.checksum.to_le_bytes().iter()),
        );
        result
    }
}

impl Ack {
    /// Creates a new instance of `Ack`.
    pub fn new(seq_num: u8) -> Ack {
        let checksum = crc16::checksum_x25(&[ACK, seq_num]);
        Ack {
            protocol: ACK,
            seq_num,
            checksum,
        }
    }
}

impl TryFrom<Bytes> for Ack {
    type Error = String;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        if value.len() == 4 {
            if value[0] == ACK {
                let checksum =
                    u16::from_le_bytes(value.split_off(value.len() - 2)[..].try_into().unwrap());

                if crc16::checksum_x25(&value) == checksum {
                    let mut protocol = value;
                    let seq_num = protocol.split_off(1);
                    let protocol = protocol[0];
                    let seq_num = seq_num[0];

                    Ok(Ack {
                        protocol,
                        seq_num,
                        checksum,
                    })
                } else {
                    Err("Invalid checksum!".into())
                }
            } else {
                Err(format!("Invalid protocol byte {} for Ack!", value.len()))
            }
        } else {
            Err(format!("Invalid length {} for Ack!", value.len()))
        }
    }
}

impl From<&AckPacket> for Ack {
    fn from(value: &AckPacket) -> Self {
        Ack::new(value.seq_num)
    }
}

impl From<Ack> for Bytes {
    fn from(value: Ack) -> Self {
        let mut result = Vec::with_capacity(4);
        result.extend(
            [value.protocol, value.seq_num]
                .iter()
                .chain(value.checksum.to_le_bytes().iter()),
        );
        result
    }
}

impl NoAckPacket {
    /// Creates a new instance of `NoAckPacket`.
    pub fn new(data: Bytes) -> NoAckPacket {
        let mut payload = Vec::with_capacity(1 + data.len());
        payload.push(NOACKPACKET);
        payload.extend(data.clone());
        let checksum = crc16::checksum_x25(&payload);
        NoAckPacket {
            protocol: NOACKPACKET,
            data,
            checksum,
        }
    }
}

impl TryFrom<Bytes> for NoAckPacket {
    type Error = String;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        if value.len() >= 4 {
            if value[0] == NOACKPACKET {
                let checksum =
                    u16::from_le_bytes(value.split_off(value.len() - 2)[..].try_into().unwrap());

                if crc16::checksum_x25(&value) == checksum {
                    let mut protocol = value;
                    let data = protocol.split_off(1);
                    let protocol = protocol[0];

                    Ok(NoAckPacket {
                        protocol,
                        data,
                        checksum,
                    })
                } else {
                    Err("Invalid checksum!".into())
                }
            } else {
                Err(format!(
                    "Invalid protocol byte {} for NoAckPacket!",
                    value.len()
                ))
            }
        } else {
            Err(format!("Invalid length {} for NoAckPacket!", value.len()))
        }
    }
}

impl From<NoAckPacket> for Bytes {
    fn from(value: NoAckPacket) -> Self {
        let mut result = Vec::with_capacity(4 + value.data.len());
        result.extend(
            [value.protocol]
                .iter()
                .chain(value.data.iter())
                .chain(value.checksum.to_le_bytes().iter()),
        );
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ackpacket_new() {
        let seq_num = 0;
        let data = vec![0x00];
        
        let packet = AckPacket::new(seq_num, data.clone());

        assert_eq!(packet.protocol, ACKPACKET);
        assert_eq!(packet.seq_num, seq_num);
        assert_eq!(packet.data, data);
        assert_eq!(packet.checksum, 0xa3db);
    }

    #[test]
    fn test_ackpacket_from_bytes() {
        let data = vec![ACKPACKET, 0x00, 0x00, 0xdb, 0xa3];
        
        let packet = AckPacket::try_from(data).unwrap();

        assert_eq!(packet.protocol, ACKPACKET);
        assert_eq!(packet.seq_num, 0x00);
        assert_eq!(packet.data, vec![0x00]);
        assert_eq!(packet.checksum, 0xa3db);
    }

    #[test]
    fn test_ackpacket_into_bytes() {
        let seq_num = 0x00;
        let packet = AckPacket::new(seq_num, vec![0x00]);

        let output = vec![ACKPACKET, seq_num, 0x00, 0xdb, 0xa3];

        assert_eq!(Bytes::from(packet), output);
    }

    #[test]
    fn test_ack_new() {
        let seq_num = 0;
        
        let packet = Ack::new(seq_num);

        assert_eq!(packet.protocol, ACK);
        assert_eq!(packet.seq_num, seq_num);
        assert_eq!(packet.checksum, 0x6349);
    }

    #[test]
    fn test_ack_from_bytes() {
        let data = vec![ACK, 0x00, 0x49, 0x63];
        
        let packet = Ack::try_from(data).unwrap();

        assert_eq!(packet.protocol, ACK);
        assert_eq!(packet.seq_num, 0x00);
        assert_eq!(packet.checksum, 0x6349);
    }

    #[test]
    fn test_ack_from_ackpacket() {
        let seq_num = 0x54;
        let ackpacket = AckPacket::new(seq_num, vec![0x01, 0x02]);

        let packet = Ack::from(&ackpacket);

        assert_eq!(packet.protocol, ACK);
        assert_eq!(packet.seq_num, seq_num);
        assert_eq!(packet.checksum, 0x77e8);
    }

    #[test]
    fn test_ack_into_bytes() {
        let seq_num = 0x00;
        let packet = Ack::new(seq_num);

        let output = vec![ACK, seq_num, 0x49, 0x63];

        assert_eq!(Bytes::from(packet), output);
    }

    #[test]
    fn test_noackpacket_new() {
        let data = vec![0x00];
        
        let packet = NoAckPacket::new(data.clone());

        assert_eq!(packet.protocol, NOACKPACKET);
        assert_eq!(packet.data, data);
        assert_eq!(packet.checksum, 0x3799);
    }

    #[test]
    fn test_noackpacket_from_bytes() {
        let data = vec![NOACKPACKET, 0x00, 0x99, 0x37];
        
        let packet = NoAckPacket::try_from(data).unwrap();

        assert_eq!(packet.protocol, NOACKPACKET);
        assert_eq!(packet.data, vec![0x00]);
        assert_eq!(packet.checksum, 0x3799);
    }

    #[test]
    fn test_noackpacket_into_bytes() {
        let packet = NoAckPacket::new(vec![0x00]);

        let output = vec![NOACKPACKET, 0x00, 0x99, 0x37];

        assert_eq!(Bytes::from(packet), output);
    }
}
