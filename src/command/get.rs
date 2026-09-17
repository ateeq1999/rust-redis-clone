use super::CommandError;
use crate::resp::types::RespType;
use crate::storage::{KeyValueStore, Value};

/// The Redis `GET` command: `GET <key>`.
#[derive(Debug)]
pub struct GetCommand {
    key: String,
}

impl GetCommand {
    /// Build a `GetCommand` from the arguments that followed `GET` in the
    /// command array (i.e. everything except the command name itself).
    pub fn from_arguments(arguments: Vec<RespType>) -> Result<GetCommand, CommandError> {
        if arguments.len() != 1 {
            return Err(CommandError::InvalidArguments(
                "ERR wrong number of arguments for 'get' command".to_string(),
            ));
        }

        let key = match arguments.into_iter().next() {
            Some(RespType::BulkString(key)) => key,
            _ => {
                return Err(CommandError::InvalidArguments(
                    "ERR GET key must be a bulk string".to_string(),
                ));
            }
        };

        Ok(GetCommand { key })
    }

    /// Look the key up and reply with its value, or a null bulk string if
    /// the key isn't set.
    pub fn execute(self, store: &KeyValueStore) -> RespType {
        match store.get(&self.key) {
            Some(Value::String(text)) => RespType::BulkString(text),
            None => RespType::NullBulkString,
        }
    }
}
