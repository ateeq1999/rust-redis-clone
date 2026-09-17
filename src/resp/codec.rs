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

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        if src.is_empty() {
            return Ok(None);
        }

        match RespType::parse(src) {
            Ok((value, consumed)) => {
                src.advance(consumed);
                Ok(Some(value))
            }
            // Not a protocol error: the rest of the value just hasn't arrived
            // yet, so leave `src` untouched and wait for more bytes.
            Err(RespError::Incomplete) => Ok(None),
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string())),
        }
    }
}

impl Encoder<RespType> for RespCodec {
    type Error = io::Error;

    fn encode(&mut self, item: RespType, dst: &mut BytesMut) -> io::Result<()> {
        dst.extend_from_slice(&item.to_bytes());
        Ok(())
    }
}
