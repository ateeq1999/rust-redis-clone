pub mod ping;

use crate::command::ping::PingCommand;
use crate::resp::types::RespType;

/// A parsed, ready-to-run Redis command.
///
/// Every RESP `Array` the server receives from a client is expected to be a
/// command: its first element names the command, the rest are its arguments.
#[derive(Debug)]
pub enum Command {
    Ping(PingCommand),
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
            _ => Err(CommandError::UnknownCommand(format!(
                "ERR unknown command '{command_name}'"
            ))),
        }
    }

    /// Run the command and produce the RESP value to send back to the client.
    pub fn execute(self) -> RespType {
        match self {
            Command::Ping(ping_command) => ping_command.execute(),
        }
    }
}
