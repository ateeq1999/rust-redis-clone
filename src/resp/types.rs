use super::RespError;
use bytes::{Bytes, BytesMut};
use std::str;

#[derive(Debug)]
pub enum RespType {
    SimpleString(String),
    BulkString(String),
    SimpleError(String),
}

impl RespType {
    /// Parse the given bytes into its respective RESP type.
    pub fn parse(buffer: &BytesMut) -> Result<(RespType, usize), RespError> {
        if buffer.is_empty() {
            return Err(RespError::Other("Empty buffer provided".to_string()));
        }

        // Match on the first byte identifier safely
        match buffer[0] {
            b'+' => Self::parse_simple_string(buffer),
            b'-' => Self::parse_simple_error(buffer),
            b'$' => Self::parse_bulk_string(buffer),
            invalid_byte => Err(RespError::Other(format!(
                "Invalid RESP data type identifier byte: '{invalid_byte}'"
            ))),
        }
    }

    /// Parse a SimpleString RESP value. Example: `+OK\r\n`
    fn parse_simple_string(buffer: &BytesMut) -> Result<(RespType, usize), RespError> {
        // Find the trailing CRLF, skipping the '+' byte at index 0
        match Self::read_till_crlf(&buffer[1..]) {
            Some((string_bytes, total_bytes_read)) => {
                match String::from_utf8(string_bytes.to_vec()) {
                    Ok(text) => {
                        // +1 to account for skipping the '+' byte initially
                        Ok((RespType::SimpleString(text), total_bytes_read + 1))
                    }
                    Err(_) => Err(RespError::InvalidSimpleString(
                        "SimpleString payload contains invalid UTF-8 sequences".to_string(),
                    )),
                }
            }
            None => Err(RespError::InvalidSimpleString(
                "Incomplete SimpleString data: Missing terminating CRLF (\\r\\n)".to_string(),
            )),
        }
    }

    /// Parse a SimpleError RESP value. Example: `-ERR unknown command\r\n`
    fn parse_simple_error(buffer: &BytesMut) -> Result<(RespType, usize), RespError> {
        // Find the trailing CRLF, skipping the '-' byte at index 0
        match Self::read_till_crlf(&buffer[1..]) {
            Some((error_bytes, total_bytes_read)) => {
                match String::from_utf8(error_bytes.to_vec()) {
                    Ok(error_msg) => {
                        // +1 to account for skipping the '-' byte initially
                        Ok((RespType::SimpleError(error_msg), total_bytes_read + 1))
                    }
                    Err(_) => Err(RespError::InvalidSimpleError(
                        "SimpleError payload contains invalid UTF-8 sequences".to_string(),
                    )),
                }
            }
            None => Err(RespError::InvalidSimpleError(
                "Incomplete SimpleError data: Missing terminating CRLF (\\r\\n)".to_string(),
            )),
        }
    }

    /// Parse a BulkString RESP value. Example: `$5\r\nhello\r\n`
    pub fn parse_bulk_string(buffer: &BytesMut) -> Result<(RespType, usize), RespError> {
        // 1. Find the first CRLF to extract the length line.
        // We look past the identifier '$' by slicing from index 1.
        let (len_bytes, length_line_size) = match Self::read_till_crlf(&buffer[1..]) {
            Some(result) => result,
            None => {
                return Err(RespError::InvalidBulkString(
                    "Missing structural length definition line or its trailing CRLF".to_string(),
                ));
            }
        };

        // 2. Parse the payload length from the extracted bytes line
        let bulkstr_len = Self::parse_usize_from_buf(len_bytes)?;

        // Calculate where the payload starts and where it should end
        let payload_start = 1 + length_line_size; // +1 accounts for the '\$' character
        let payload_end = payload_start + bulkstr_len;
        let total_expected_bytes = payload_end + 2; // +2 accounts for trailing payload CRLF

        // 3. Ensure the buffer holds the full payload AND its trailing CRLF
        if buffer.len() < total_expected_bytes {
            return Err(RespError::InvalidBulkString(format!(
                "Incomplete buffer payload: Expected at least {} bytes but buffer only contains {} bytes",
                total_expected_bytes,
                buffer.len()
            )));
        }

        // 4. Validate that the payload is properly followed by the trailing CRLF sequence
        if &buffer[payload_end..total_expected_bytes] != b"\r\n" {
            return Err(RespError::InvalidBulkString(
                "Malformed bulk string framework: Missing expected trailing CRLF (\\r\\n) directly after string data".to_string()
            ));
        }

        // 5. Convert raw payload bytes into a UTF-8 String
        let payload_bytes = &buffer[payload_start..payload_end];
        match String::from_utf8(payload_bytes.to_vec()) {
            Ok(bulkstr) => Ok((RespType::BulkString(bulkstr), total_expected_bytes)),
            Err(_) => Err(RespError::InvalidBulkString(
                "Bulk string payload contains invalid UTF-8 string data".to_string(),
            )),
        }
    }

    /// Finds the index of the first CRLF ("\r\n").
    /// Returns a tuple containing the slice *before* the CRLF, and the total bytes consumed up to and including CRLF.
    fn read_till_crlf(buf: &[u8]) -> Option<(&[u8], usize)> {
        for (index, window) in buf.windows(2).enumerate() {
            if window == b"\r\n" {
                return Some((&buf[..index], index + 2));
            }
        }
        None
    }

    /// Converts raw ASCII bytes directly into a `usize`.
    fn parse_usize_from_buf(buf: &[u8]) -> Result<usize, RespError> {
        match str::from_utf8(buf) {
            Ok(str_slice) => match str_slice.parse::<usize>() {
                Ok(parsed_int) => Ok(parsed_int),
                Err(_) => Err(RespError::Other(format!(
                    "Failed to convert numerical field to an unsigned integer: '{str_slice}'"
                ))),
            },
            Err(_) => Err(RespError::Other(
                "Length configuration prefix line contains invalid non-UTF-8 characters"
                    .to_string(),
            )),
        }
    }

    /// Convert the RESP value into its byte values.
    pub fn to_bytes(&self) -> Bytes {
        match self {
            RespType::SimpleString(ss) => Bytes::from_iter(format!("+{}\r\n", ss).into_bytes()),
            RespType::BulkString(bs) => {
                let bulkstr_bytes = format!("${}\r\n{}\r\n", bs.chars().count(), bs).into_bytes();
                Bytes::from_iter(bulkstr_bytes)
            }
            RespType::SimpleError(es) => Bytes::from_iter(format!("-{}\r\n", es).into_bytes()),
        }
    }
}
