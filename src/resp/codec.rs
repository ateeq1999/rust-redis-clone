use super::{RespError, types::RespType};
use bytes::{Buf, BytesMut};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

/// Frames a byte stream into RESP values so a connection can be driven
/// through `tokio_util::codec::Framed` instead of manual buffer handling.
#[derive(Debug, Default)]
pub struct RespCodec;

impl Decoder for RespCodec {
    type Item = RespType;
    type Error = io::Error;

    fn decode(&mut self, read_buffer: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        if read_buffer.is_empty() {
            return Ok(None);
        }

        match RespType::parse(read_buffer) {
            Ok((value, bytes_consumed)) => {
                read_buffer.advance(bytes_consumed);
                Ok(Some(value))
            }
            // Not a protocol error: the rest of the value just hasn't arrived
            // yet, so leave `read_buffer` untouched and wait for more bytes.
            Err(RespError::Incomplete) => Ok(None),
            Err(parse_error) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                parse_error.to_string(),
            )),
        }
    }
}

impl Encoder<RespType> for RespCodec {
    type Error = io::Error;

    fn encode(&mut self, value: RespType, write_buffer: &mut BytesMut) -> io::Result<()> {
        write_buffer.extend_from_slice(&value.to_bytes());
        Ok(())
    }
}
