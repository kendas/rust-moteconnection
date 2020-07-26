//! The [HDLC][1] implementation used in `moteconnection` lives here.
//!
//! [1]: https://github.com/proactivity-lab/docs/wiki/Serial-protocol#hdlc-framing-and-escaping
use crate::Bytes;

const FRAME: u8 = 0x7E;
const ESCAPE: u8 = 0x7D;
const ESCAPED_FRAME: u8 = 0x5E;
const ESCAPED_ESCAPE: u8 = 0x5D;
const MASK: u8 = 0x20;

/// Provides an interface to encode/decode HDLC-framed byte strings.
///
/// Supports decoding in chunks.
pub struct HdlcCodec {
    decoded: Option<Bytes>,
    escaped: bool,
}

impl HdlcCodec {
    /// Creates a new `HdlcCodec` instance.
    pub fn new() -> HdlcCodec {
        HdlcCodec::default()
    }

    // vec![0x7E, 0x44, 0x00, 0x0E, 0x7D, 0x5D, 0x7D],
    // vec![0x5D, 0x7D, 0x5D, 0xED, 0xB9, 0x7E],
    /// Attempts to decode a HDLC-encoded stream.
    ///
    /// Supports decoding a HDLC-encoded stream in chunks.
    pub fn decode(&mut self, input: &[u8]) -> Vec<Bytes> {
        let mut result = Vec::new();
        for byte in input {
            self.decoded = if self.escaped {
                self.escaped = false;
                match *byte {
                    ESCAPED_FRAME => self.decoded.take().map(|mut buffer| {
                        buffer.push(FRAME);
                        buffer
                    }),
                    ESCAPED_ESCAPE => self.decoded.take().map(|mut buffer| {
                        buffer.push(ESCAPE);
                        buffer
                    }),
                    FRAME => Some(Vec::new()),
                    _ => {
                        self.decoded.take();
                        None
                    }
                }
            } else {
                match *byte {
                    FRAME => {
                        if let Some(buffer) = self.decoded.take() {
                            if !buffer.is_empty() {
                                result.push(buffer);
                            }
                        }
                        Some(Vec::new())
                    }
                    ESCAPE => {
                        self.escaped = true;
                        self.decoded.take()
                    }
                    value => self.decoded.take().map(|mut buffer| {
                        buffer.push(value);
                        buffer
                    }),
                }
            }
        }
        result
    }

    /// Encodes a byte string in HDLC encoding.
    ///
    /// # Example:
    ///
    /// ```rust
    /// use moteconnection::transport::serial::hdlc::HdlcCodec;
    ///
    /// let input = vec![0x44, 0x00, 0xFF, 0x9D, 0xDF];
    /// let output = HdlcCodec::encode(&input);
    /// assert_eq!(output, vec![0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E]);
    /// ```
    pub fn encode(input: &[u8]) -> Bytes {
        input
            .iter()
            .fold(vec![FRAME], |mut acc, byte| {
                if [FRAME, ESCAPE].contains(byte) {
                    acc.push(ESCAPE);
                    acc.push(byte ^ MASK);
                } else {
                    acc.push(*byte);
                }
                acc
            })
            .into_iter()
            .chain(vec![FRAME])
            .collect()
    }
}

impl Default for HdlcCodec {
    fn default() -> HdlcCodec {
        HdlcCodec {
            decoded: None,
            escaped: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_encode() {
        let input = vec![0x44, 0x00, 0xFF, 0x9D, 0xDF];
        let output = HdlcCodec::encode(&input);

        assert_eq!(output, vec![0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E]);

        let input = vec![
            0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
            0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B,
        ];
        let output = HdlcCodec::encode(&input);

        assert_eq!(
            output,
            vec![
                0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
                0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E
            ]
        );
    }

    #[test]
    fn test_framing_byte_encode() {
        let input = vec![0x7E];
        let output = HdlcCodec::encode(&input);

        assert_eq!(output, vec![0x7E, 0x7D, 0x5E, 0x7E]);

        let input = vec![0x44, 0x00, 0x0E, 0x7E, 0x7E, 0x7E, 0xED, 0xB9];
        let output = HdlcCodec::encode(&input);

        assert_eq!(
            output,
            vec![0x7E, 0x44, 0x00, 0x0E, 0x7D, 0x5E, 0x7D, 0x5E, 0x7D, 0x5E, 0xED, 0xB9, 0x7E,]
        );
    }

    #[test]
    fn test_escape_byte_encode() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x7D, 0x5D, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x7D]);

        let input = vec![
            0x7E, 0x44, 0x00, 0x0E, 0x7D, 0x5D, 0x7D, 0x5D, 0x7D, 0x5D, 0xED, 0xB9, 0x7E,
        ];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0],
            vec![0x44, 0x00, 0x0E, 0x7D, 0x7D, 0x7D, 0xED, 0xB9]
        );
    }

    #[test]
    fn test_simple_decode() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x44, 0x00, 0xFF, 0x9D, 0xDF]);

        let input = vec![
            0x7E, 0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B, 0x7E,
        ];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0],
            vec![
                0x44, 0x00, 0x0E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
                0x0C, 0x0D, 0x0E, 0x0F, 0x3B, 0x8B,
            ]
        );
    }

    #[test]
    fn test_framing_byte_decode() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x7D, 0x5E, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x7E]);

        let input = vec![
            0x7E, 0x44, 0x00, 0x0E, 0x7D, 0x5E, 0x7D, 0x5E, 0x7D, 0x5E, 0xED, 0xB9, 0x7E,
        ];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0],
            vec![0x44, 0x00, 0x0E, 0x7E, 0x7E, 0x7E, 0xED, 0xB9]
        );
    }

    #[test]
    fn test_escape_byte_decode() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x7D, 0x5D, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x7D]);

        let input = vec![
            0x7E, 0x44, 0x00, 0x0E, 0x7D, 0x5D, 0x7D, 0x5D, 0x7D, 0x5D, 0xED, 0xB9, 0x7E,
        ];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0],
            vec![0x44, 0x00, 0x0E, 0x7D, 0x7D, 0x7D, 0xED, 0xB9]
        );
    }

    #[test]
    fn test_multiple_framing_bytes() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x7E, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 0);

        let input = vec![0x7E, 0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x44, 0x00, 0xFF, 0x9D, 0xDF]);

        let input = vec![0x7E, 0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x44, 0x00, 0xFF, 0x9D, 0xDF]);
    }

    #[test]
    fn test_many_frames() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x5E, 0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 2);
        assert_eq!(output[0], vec![0x5E]);
        assert_eq!(output[1], vec![0x44, 0x00, 0xFF, 0x9D, 0xDF]);
    }

    #[test]
    fn test_no_frame_at_start() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x5E, 0x7E, 0x44, 0x00, 0xFF, 0x9D, 0xDF, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 1);
        assert_eq!(output[0], vec![0x44, 0x00, 0xFF, 0x9D, 0xDF]);
    }

    #[test]
    fn test_chunked() {
        let mut codec = HdlcCodec::new();

        let input = (
            vec![0x7E, 0x44, 0x00, 0x0E, 0x7D, 0x5D, 0x7D],
            vec![0x5D, 0x7D, 0x5D, 0xED, 0xB9, 0x7E],
        );
        let output = codec.decode(&input.0);

        assert_eq!(output.len(), 0);

        let output = codec.decode(&input.1);

        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0],
            vec![0x44, 0x00, 0x0E, 0x7D, 0x7D, 0x7D, 0xED, 0xB9]
        );
    }

    #[test]
    fn test_invalid_escape() {
        let mut codec = HdlcCodec::new();

        let input = vec![0x7E, 0x7D, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 0);

        let input = vec![0x7E, 0x7D, 0x7D, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 0);

        let input = vec![0x7E, 0x7D, 0x01, 0x7E];
        let output = codec.decode(&input);

        assert_eq!(output.len(), 0);
    }
}
