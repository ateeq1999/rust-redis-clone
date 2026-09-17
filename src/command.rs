pub mod get;
pub mod lrange;
pub mod ping;
pub mod push;
pub mod set;

use crate::command::get::GetCommand;
use crate::command::lrange::LRangeCommand;
use crate::command::ping::PingCommand;
use crate::command::push::{LPushCommand, RPushCommand};
use crate::command::set::SetCommand;
use crate::resp::types::RespType;
use crate::storage::KeyValueStore;

/// A parsed, ready-to-run Redis command.
///
/// Every RESP `Array` the server receives from a client is expected to be a
/// command: its first element names the command, the rest are its arguments.
#[derive(Debug)]
pub enum Command {
    Ping(PingCommand),
    Set(SetCommand),
    Get(GetCommand),
    LPush(LPushCommand),
    RPush(RPushCommand),
    LRange(LRangeCommand),
}

/// Represents errors that can occur while turning a RESP array into a `Command`.
#[derive(Debug)]
pub enum CommandError {
    /// The array wasn't shaped like a valid command (e.g. empty, or the
    /// command name wasn't a bulk string).
    InvalidCommandFormat(String),
    /// The command name doesn't match any command this server implements.
    UnknownCommand(String),
    /// A recognized command was called with the wrong number or type of arguments.
    InvalidArguments(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::InvalidCommandFormat(message) => message.as_str().fmt(formatter),
            CommandError::UnknownCommand(message) => message.as_str().fmt(formatter),
            CommandError::InvalidArguments(message) => message.as_str().fmt(formatter),
        }
    }
}

/// Renders a `CommandError` the way a real client would see it: as a RESP
/// SimpleError, so it can be sent straight back over the wire.
impl From<CommandError> for RespType {
    fn from(error: CommandError) -> Self {
        RespType::SimpleError(error.to_string())
    }
}

impl Command {
    /// Interpret a decoded RESP array as a command, dispatching on its first
    /// element (the command name) and handing the rest to that command's
    /// own argument parser.
    pub fn from_array_elements(elements: Vec<RespType>) -> Result<Command, CommandError> {
        let mut elements = elements.into_iter();

        let command_name = match elements.next() {
            Some(RespType::BulkString(name)) => name,
            Some(_) => {
                return Err(CommandError::InvalidCommandFormat(
                    "ERR command name must be a bulk string".to_string(),
                ));
            }
            None => {
                return Err(CommandError::InvalidCommandFormat(
                    "ERR empty command array".to_string(),
                ));
            }
        };
        let command_arguments: Vec<RespType> = elements.collect();

        match command_name.to_lowercase().as_str() {
            "ping" => Ok(Command::Ping(PingCommand::from_arguments(
                command_arguments,
            )?)),
            "set" => Ok(Command::Set(SetCommand::from_arguments(
                command_arguments,
            )?)),
            "get" => Ok(Command::Get(GetCommand::from_arguments(
                command_arguments,
            )?)),
            "lpush" => Ok(Command::LPush(LPushCommand::from_arguments(
                command_arguments,
            )?)),
            "rpush" => Ok(Command::RPush(RPushCommand::from_arguments(
                command_arguments,
            )?)),
            "lrange" => Ok(Command::LRange(LRangeCommand::from_arguments(
                command_arguments,
            )?)),
            _ => Err(CommandError::UnknownCommand(format!(
                "ERR unknown command '{command_name}'"
            ))),
        }
    }

    /// Run the command against the shared store and produce the RESP value
    /// to send back to the client.
    pub fn execute(self, store: &KeyValueStore) -> RespType {
        match self {
            Command::Ping(ping_command) => ping_command.execute(),
            Command::Set(set_command) => set_command.execute(store),
            Command::Get(get_command) => get_command.execute(store),
            Command::LPush(lpush_command) => lpush_command.execute(store),
            Command::RPush(rpush_command) => rpush_command.execute(store),
            Command::LRange(lrange_command) => lrange_command.execute(store),
        }
    }
}
