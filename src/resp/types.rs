use super::RespError;
use bytes::Bytes;
use std::str;

#[derive(Debug)]
pub enum RespType {
    SimpleString(String),
    BulkString(String),
    SimpleError(String),
    Array(Vec<RespType>),
    /// The RESP encoding of "no value" (e.g. `GET` on a missing key):
    /// a bulk string with length `-1` and no payload, `$-1\r\n`.
    NullBulkString,
}

impl RespType {
    /// Parse the given bytes into its respective RESP type.
    pub fn parse(buffer: &[u8]) -> Result<(RespType, usize), RespError> {
        if buffer.is_empty() {
            return Err(RespError::Incomplete);
        }

        // Match on the first byte identifier safely
        match buffer[0] {
            b'+' => Self::parse_simple_string(buffer),
            b'-' => Self::parse_simple_error(buffer),
            b'$' => Self::parse_bulk_string(buffer),
            b'*' => Self::parse_array(buffer),
            invalid_byte => Err(RespError::Other(format!(
                "Invalid RESP data type identifier byte: '{invalid_byte}'"
            ))),
        }
    }

    /// Parse a SimpleString RESP value. Example: `+OK\r\n`
    fn parse_simple_string(buffer: &[u8]) -> Result<(RespType, usize), RespError> {
        // Find the trailing CRLF, skipping the '+' byte at index 0
        match Self::read_till_crlf(&buffer[1..]) {
            Some((text_bytes, bytes_consumed)) => match String::from_utf8(text_bytes.to_vec()) {
                Ok(text) => {
                    // +1 to account for skipping the '+' byte initially
                    Ok((RespType::SimpleString(text), bytes_consumed + 1))
                }
                Err(_) => Err(RespError::InvalidSimpleString(
                    "SimpleString payload contains invalid UTF-8 sequences".to_string(),
                )),
            },
            // No CRLF yet: this may just mean the rest is still in flight.
            None => Err(RespError::Incomplete),
        }
    }

    /// Parse a SimpleError RESP value. Example: `-ERR unknown command\r\n`
    fn parse_simple_error(buffer: &[u8]) -> Result<(RespType, usize), RespError> {
        // Find the trailing CRLF, skipping the '-' byte at index 0
        match Self::read_till_crlf(&buffer[1..]) {
            Some((message_bytes, bytes_consumed)) => {
                match String::from_utf8(message_bytes.to_vec()) {
                    Ok(error_message) => {
                        // +1 to account for skipping the '-' byte initially
                        Ok((RespType::SimpleError(error_message), bytes_consumed + 1))
                    }
                    Err(_) => Err(RespError::InvalidSimpleError(
                        "SimpleError payload contains invalid UTF-8 sequences".to_string(),
                    )),
                }
            }
            None => Err(RespError::Incomplete),
        }
    }

    /// Parse a BulkString RESP value. Example: `$5\r\nhello\r\n`
    pub fn parse_bulk_string(buffer: &[u8]) -> Result<(RespType, usize), RespError> {
        // 1. Find the first CRLF to extract the length line.
        // We look past the identifier '$' by slicing from index 1.
        let (length_line_bytes, length_line_byte_count) = match Self::read_till_crlf(&buffer[1..])
        {
            Some(result) => result,
            None => return Err(RespError::Incomplete),
        };

        // A length of -1 denotes a null bulk string ("$-1\r\n"): unlike every
        // other bulk string, it has no payload or trailing CRLF of its own -
        // the length line above is the entire value.
        if length_line_bytes == b"-1" {
            return Ok((RespType::NullBulkString, 1 + length_line_byte_count));
        }

        // 2. Parse the payload length from the extracted bytes line
        let bulk_string_length = Self::parse_usize_from_bytes(length_line_bytes)?;

        // Calculate where the payload starts and where it should end
        let payload_start = 1 + length_line_byte_count; // +1 accounts for the '$' character
        let payload_end = payload_start + bulk_string_length;
        let total_expected_bytes = payload_end + 2; // +2 accounts for trailing payload CRLF

        // 3. Ensure the buffer holds the full payload AND its trailing CRLF
        if buffer.len() < total_expected_bytes {
            return Err(RespError::Incomplete);
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
            Ok(bulk_string_text) => Ok((RespType::BulkString(bulk_string_text), total_expected_bytes)),
            Err(_) => Err(RespError::InvalidBulkString(
                "Bulk string payload contains invalid UTF-8 string data".to_string(),
            )),
        }
    }

    /// Parse an Array RESP value. Example: `*2\r\n$4\r\nPING\r\n$4\r\npong\r\n`
    ///
    /// Redis commands are sent as arrays of bulk strings, so this recurses
    /// into `parse` for each element rather than assuming a specific type.
    fn parse_array(buffer: &[u8]) -> Result<(RespType, usize), RespError> {
        // 1. Find the first CRLF to extract the element-count line.
        let (length_line_bytes, length_line_byte_count) = match Self::read_till_crlf(&buffer[1..])
        {
            Some(result) => result,
            None => return Err(RespError::Incomplete),
        };

        // 2. Parse the number of elements the array should contain
        let element_count = Self::parse_usize_from_bytes(length_line_bytes)?;

        // +1 accounts for the leading '*' byte
        let mut bytes_consumed = 1 + length_line_byte_count;
        let mut elements = Vec::with_capacity(element_count);

        // 3. Parse each element in turn, advancing past whatever bytes it consumed.
        // Any nested `Incomplete` bubbles straight up: the whole array is only
        // ready once every element is fully buffered.
        for _ in 0..element_count {
            let (element, element_byte_count) = Self::parse(&buffer[bytes_consumed..])?;
            elements.push(element);
            bytes_consumed += element_byte_count;
        }

        Ok((RespType::Array(elements), bytes_consumed))
    }

    /// Finds the index of the first CRLF ("\r\n").
    /// Returns a tuple containing the slice *before* the CRLF, and the total bytes consumed up to and including CRLF.
    fn read_till_crlf(bytes: &[u8]) -> Option<(&[u8], usize)> {
        for (index, window) in bytes.windows(2).enumerate() {
            if window == b"\r\n" {
                return Some((&bytes[..index], index + 2));
            }
        }
        None
    }

    /// Converts raw ASCII digit bytes directly into a `usize`.
    fn parse_usize_from_bytes(digit_bytes: &[u8]) -> Result<usize, RespError> {
        match str::from_utf8(digit_bytes) {
            Ok(digits) => match digits.parse::<usize>() {
                Ok(parsed_value) => Ok(parsed_value),
                Err(_) => Err(RespError::Other(format!(
                    "Failed to convert numerical field to an unsigned integer: '{digits}'"
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
            RespType::SimpleString(text) => Bytes::from_iter(format!("+{}\r\n", text).into_bytes()),
            RespType::BulkString(payload) => {
                let bulk_string_bytes =
                    format!("${}\r\n{}\r\n", payload.chars().count(), payload).into_bytes();
                Bytes::from_iter(bulk_string_bytes)
            }
            RespType::SimpleError(error_message) => {
                Bytes::from_iter(format!("-{}\r\n", error_message).into_bytes())
            }
            RespType::Array(elements) => {
                let mut array_bytes = format!("*{}\r\n", elements.len()).into_bytes();
                for element in elements {
                    array_bytes.extend_from_slice(&element.to_bytes());
                }
                Bytes::from_iter(array_bytes)
            }
            RespType::NullBulkString => Bytes::from_static(b"$-1\r\n"),
        }
    }
}
