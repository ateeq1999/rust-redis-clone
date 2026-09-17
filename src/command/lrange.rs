use super::CommandError;
use crate::resp::types::RespType;
use crate::storage::KeyValueStore;

/// The Redis `LRANGE` command: `LRANGE <key> <start> <stop>`.
///
/// `start`/`stop` are zero-based indices into the list, both inclusive;
/// negative indices count from the end (`-1` is the last element).
/// Out-of-range indices are clamped rather than treated as errors, and a
/// missing key behaves like an empty list - matching real Redis.
#[derive(Debug)]
pub struct LRangeCommand {
    key: String,
    start: i64,
    stop: i64,
}

impl LRangeCommand {
    /// Build an `LRangeCommand` from the arguments that followed `LRANGE` in
    /// the command array (i.e. everything except the command name itself).
    pub fn from_arguments(arguments: Vec<RespType>) -> Result<LRangeCommand, CommandError> {
        if arguments.len() != 3 {
            return Err(CommandError::InvalidArguments(
                "ERR wrong number of arguments for 'lrange' command".to_string(),
            ));
        }

        let mut arguments = arguments.into_iter();

        let key = match arguments.next() {
            Some(RespType::BulkString(key)) => key,
            _ => {
                return Err(CommandError::InvalidArguments(
                    "ERR LRANGE key must be a bulk string".to_string(),
                ));
            }
        };

        let start = parse_index_argument(arguments.next())?;
        let stop = parse_index_argument(arguments.next())?;

        Ok(LRangeCommand { key, start, stop })
    }

    /// Look up the range and reply with it as an array of bulk strings.
    pub fn execute(self, store: &KeyValueStore) -> RespType {
        match store.range(&self.key, self.start, self.stop) {
            Ok(values) => {
                RespType::Array(values.into_iter().map(RespType::BulkString).collect())
            }
            Err(error) => RespType::SimpleError(error.to_string()),
        }
    }
}

/// Parses an `LRANGE` `start`/`stop` argument: a bulk string holding an `i64`.
fn parse_index_argument(argument: Option<RespType>) -> Result<i64, CommandError> {
    match argument {
        Some(RespType::BulkString(text)) => text.parse::<i64>().map_err(|_| {
            CommandError::InvalidArguments(
                "ERR value is not an integer or out of range".to_string(),
            )
        }),
        _ => Err(CommandError::InvalidArguments(
            "ERR LRANGE start/stop must be a bulk string integer".to_string(),
        )),
    }
}
