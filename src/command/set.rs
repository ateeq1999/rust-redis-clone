use super::CommandError;
use crate::resp::types::RespType;
use crate::storage::{KeyValueStore, Value};

/// The Redis `SET` command: `SET <key> <value>`.
///
/// Advanced options (`EX`, `PX`, `NX`, ...) aren't supported yet - a `SET`
/// with anything other than exactly a key and a value is rejected outright
/// rather than silently ignoring the extra arguments.
#[derive(Debug)]
pub struct SetCommand {
    key: String,
    value: String,
}

impl SetCommand {
    /// Build a `SetCommand` from the arguments that followed `SET` in the
    /// command array (i.e. everything except the command name itself).
    pub fn from_arguments(arguments: Vec<RespType>) -> Result<SetCommand, CommandError> {
        if arguments.len() != 2 {
            return Err(CommandError::InvalidArguments(
                "ERR wrong number of arguments for 'set' command".to_string(),
            ));
        }

        let mut arguments = arguments.into_iter();

        let key = match arguments.next() {
            Some(RespType::BulkString(key)) => key,
            _ => {
                return Err(CommandError::InvalidArguments(
                    "ERR SET key must be a bulk string".to_string(),
                ));
            }
        };

        let value = match arguments.next() {
            Some(RespType::BulkString(value)) => value,
            _ => {
                return Err(CommandError::InvalidArguments(
                    "ERR SET value must be a bulk string".to_string(),
                ));
            }
        };

        Ok(SetCommand { key, value })
    }

    /// Store the key/value pair and reply the way real Redis does: `+OK`.
    pub fn execute(self, store: &KeyValueStore) -> RespType {
        store.set(self.key, Value::String(self.value));
        RespType::SimpleString("OK".to_string())
    }
}
