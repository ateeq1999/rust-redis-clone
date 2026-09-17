use super::CommandError;
use crate::resp::types::RespType;
use crate::storage::KeyValueStore;

/// The Redis `LPUSH` command: `LPUSH <key> <value> [value ...]` - pushes
/// each value onto the head (front) of the list, creating it if needed.
#[derive(Debug)]
pub struct LPushCommand {
    key: String,
    values: Vec<String>,
}

impl LPushCommand {
    /// Build an `LPushCommand` from the arguments that followed `LPUSH` in
    /// the command array (i.e. everything except the command name itself).
    pub fn from_arguments(arguments: Vec<RespType>) -> Result<LPushCommand, CommandError> {
        let (key, values) = parse_key_and_values("lpush", arguments)?;
        Ok(LPushCommand { key, values })
    }

    /// Push the values and reply with the list's new length.
    pub fn execute(self, store: &KeyValueStore) -> RespType {
        match store.push_front(self.key, self.values) {
            Ok(length) => RespType::Integer(length as i64),
            Err(error) => RespType::SimpleError(error.to_string()),
        }
    }
}

/// The Redis `RPUSH` command: `RPUSH <key> <value> [value ...]` - pushes
/// each value onto the tail (back) of the list, creating it if needed.
#[derive(Debug)]
pub struct RPushCommand {
    key: String,
    values: Vec<String>,
}

impl RPushCommand {
    /// Build an `RPushCommand` from the arguments that followed `RPUSH` in
    /// the command array (i.e. everything except the command name itself).
    pub fn from_arguments(arguments: Vec<RespType>) -> Result<RPushCommand, CommandError> {
        let (key, values) = parse_key_and_values("rpush", arguments)?;
        Ok(RPushCommand { key, values })
    }

    /// Push the values and reply with the list's new length.
    pub fn execute(self, store: &KeyValueStore) -> RespType {
        match store.push_back(self.key, self.values) {
            Ok(length) => RespType::Integer(length as i64),
            Err(error) => RespType::SimpleError(error.to_string()),
        }
    }
}

/// Shared argument parsing for `LPUSH`/`RPUSH`: `<key> <value> [value ...]`.
/// `command_name` (already lowercase) is only used to word the error
/// messages for whichever of the two commands is being parsed.
fn parse_key_and_values(
    command_name: &str,
    arguments: Vec<RespType>,
) -> Result<(String, Vec<String>), CommandError> {
    if arguments.len() < 2 {
        return Err(CommandError::InvalidArguments(format!(
            "ERR wrong number of arguments for '{command_name}' command"
        )));
    }

    let mut arguments = arguments.into_iter();

    let key = match arguments.next() {
        Some(RespType::BulkString(key)) => key,
        _ => {
            return Err(CommandError::InvalidArguments(format!(
                "ERR {} key must be a bulk string",
                command_name.to_uppercase()
            )));
        }
    };

    let mut values = Vec::with_capacity(arguments.len());
    for argument in arguments {
        match argument {
            RespType::BulkString(value) => values.push(value),
            _ => {
                return Err(CommandError::InvalidArguments(format!(
                    "ERR {} values must be bulk strings",
                    command_name.to_uppercase()
                )));
            }
        }
    }

    Ok((key, values))
}
