use std::fmt;

/// A decoded RESP value, as seen from the client side of the wire.
///
/// This mirrors the server's own `RespType` (see `src/resp/types.rs`), but
/// lives here as its own copy since an example binary can't reach into the
/// main crate's private modules - there's no library target to import from.
#[derive(Debug, Clone)]
pub enum RespValue {
    SimpleString(String),
    BulkString(String),
    Error(String),
    Integer(i64),
    NullBulkString,
    Array(Vec<RespValue>),
}

impl RespValue {
    /// Decode one RESP value from the front of `buffer`, returning it along
    /// with how many bytes it occupied. Unlike the server's parser, this
    /// doesn't need to distinguish "incomplete" from "invalid" - the client
    /// already has a full response in hand before decoding it.
    pub fn decode(buffer: &[u8]) -> Result<(RespValue, usize), String> {
        match buffer.first() {
            None => Err("empty RESP response".to_string()),
            Some(b'+') => {
                let (text, bytes_consumed) = decode_line(buffer)?;
                Ok((RespValue::SimpleString(text), bytes_consumed))
            }
            Some(b'-') => {
                let (text, bytes_consumed) = decode_line(buffer)?;
                Ok((RespValue::Error(text), bytes_consumed))
            }
            Some(b':') => {
                let (text, bytes_consumed) = decode_line(buffer)?;
                let value = text
                    .parse::<i64>()
                    .map_err(|_| format!("invalid RESP integer: '{text}'"))?;
                Ok((RespValue::Integer(value), bytes_consumed))
            }
            Some(b'$') => decode_bulk_string(buffer),
            Some(b'*') => decode_array(buffer),
            Some(other) => Err(format!(
                "unrecognized RESP type byte: '{}'",
                *other as char
            )),
        }
    }

    /// Render this value as JSON. There's no dependency on `serde_json` here
    /// on purpose, to keep this example self-contained - it's a small enough
    /// value space (strings, integers, null, arrays, and an error wrapper)
    /// that hand-rolling it is straightforward.
    pub fn to_json(&self) -> String {
        match self {
            RespValue::SimpleString(text) => json_string(text),
            RespValue::BulkString(text) => json_string(text),
            RespValue::Integer(value) => value.to_string(),
            RespValue::NullBulkString => "null".to_string(),
            RespValue::Error(message) => format!(r#"{{"error":{}}}"#, json_string(message)),
            RespValue::Array(elements) => {
                let items: Vec<String> = elements.iter().map(RespValue::to_json).collect();
                format!("[{}]", items.join(","))
            }
        }
    }
}

/// Prints a value the way `redis-cli` would: a bare `PONG` for simple
/// strings, a quoted `"bar"` for bulk strings, `(nil)` for a null bulk
/// string, `(integer) 3` for integers, `(error) ...` for errors, and a
/// numbered list for arrays.
impl fmt::Display for RespValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RespValue::SimpleString(text) => write!(formatter, "{text}"),
            RespValue::BulkString(text) => write!(formatter, "{text:?}"),
            RespValue::Integer(value) => write!(formatter, "(integer) {value}"),
            RespValue::NullBulkString => write!(formatter, "(nil)"),
            RespValue::Error(message) => write!(formatter, "(error) {message}"),
            RespValue::Array(elements) if elements.is_empty() => write!(formatter, "(empty array)"),
            RespValue::Array(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        writeln!(formatter)?;
                    }
                    write!(formatter, "{}) {}", index + 1, element)?;
                }
                Ok(())
            }
        }
    }
}

/// Decodes a `+...\r\n`/`-...\r\n`/`:...\r\n` line: everything up to (but
/// not including) the trailing CRLF, plus the total bytes consumed.
fn decode_line(buffer: &[u8]) -> Result<(String, usize), String> {
    match read_till_crlf(&buffer[1..]) {
        Some((text_bytes, bytes_consumed)) => {
            let text = String::from_utf8(text_bytes.to_vec())
                .map_err(|_| "RESP line contains invalid UTF-8".to_string())?;
            Ok((text, bytes_consumed + 1))
        }
        None => Err("RESP line is missing its terminating CRLF".to_string()),
    }
}

/// Decodes a BulkString (`$5\r\nhello\r\n`) or a null bulk string (`$-1\r\n`).
fn decode_bulk_string(buffer: &[u8]) -> Result<(RespValue, usize), String> {
    let (length_line, length_line_byte_count) = read_till_crlf(&buffer[1..])
        .ok_or_else(|| "bulk string is missing its length line".to_string())?;

    if length_line == b"-1" {
        return Ok((RespValue::NullBulkString, 1 + length_line_byte_count));
    }

    let length: usize = std::str::from_utf8(length_line)
        .ok()
        .and_then(|digits| digits.parse().ok())
        .ok_or_else(|| "bulk string has an invalid length".to_string())?;

    let payload_start = 1 + length_line_byte_count;
    let payload_end = payload_start + length;
    let total_bytes = payload_end + 2;

    if buffer.len() < total_bytes {
        return Err("bulk string payload is shorter than its declared length".to_string());
    }

    let text = String::from_utf8(buffer[payload_start..payload_end].to_vec())
        .map_err(|_| "bulk string payload contains invalid UTF-8".to_string())?;

    Ok((RespValue::BulkString(text), total_bytes))
}

/// Decodes an Array (`*2\r\n$4\r\nECHO\r\n$5\r\nhello\r\n`) by recursing into
/// `RespValue::decode` for each element.
fn decode_array(buffer: &[u8]) -> Result<(RespValue, usize), String> {
    let (length_line, length_line_byte_count) = read_till_crlf(&buffer[1..])
        .ok_or_else(|| "array is missing its length line".to_string())?;

    let element_count: usize = std::str::from_utf8(length_line)
        .ok()
        .and_then(|digits| digits.parse().ok())
        .ok_or_else(|| "array has an invalid element count".to_string())?;

    let mut bytes_consumed = 1 + length_line_byte_count;
    let mut elements = Vec::with_capacity(element_count);

    for _ in 0..element_count {
        let (element, element_byte_count) = RespValue::decode(&buffer[bytes_consumed..])?;
        elements.push(element);
        bytes_consumed += element_byte_count;
    }

    Ok((RespValue::Array(elements), bytes_consumed))
}

/// Finds the first CRLF, returning the slice before it and the total bytes
/// consumed (including the CRLF itself).
fn read_till_crlf(bytes: &[u8]) -> Option<(&[u8], usize)> {
    for (index, window) in bytes.windows(2).enumerate() {
        if window == b"\r\n" {
            return Some((&bytes[..index], index + 2));
        }
    }
    None
}

/// Encodes `text` as a JSON string literal, including the surrounding quotes.
fn json_string(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len() + 2);
    encoded.push('"');
    for character in text.chars() {
        match character {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                encoded.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => encoded.push(other),
        }
    }
    encoded.push('"');
    encoded
}
