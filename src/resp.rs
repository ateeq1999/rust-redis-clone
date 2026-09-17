pub mod codec;
pub mod types;

/// Represents errors that can occur during RESP parsing.
#[derive(Debug)]
pub enum RespError {
    /// The buffer doesn't yet hold a full RESP value. Not a protocol error —
    /// callers reading from a stream (see `codec`) should wait for more bytes.
    Incomplete,
    /// Represents an error in parsing a bulk string, with an error message.
    InvalidBulkString(String),
    /// Represents an error in parsing a simple string, with an error message.
    InvalidSimpleString(String),
    /// Represents an error in parsing a simple error, with an error message.
    InvalidSimpleError(String),
    /// Represents any other error with a descriptive message.
    Other(String),
}

impl std::fmt::Display for RespError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RespError::Incomplete => "Incomplete RESP data: waiting for more bytes".fmt(formatter),
            RespError::Other(message) => message.as_str().fmt(formatter),
            RespError::InvalidBulkString(message) => message.as_str().fmt(formatter),
            RespError::InvalidSimpleString(message) => message.as_str().fmt(formatter),
            RespError::InvalidSimpleError(message) => message.as_str().fmt(formatter),
        }
    }
}
