use super::CommandError;
use crate::resp::types::RespType;

/// The Redis `PING` command: `PING` or `PING <message>`.
///
/// With no message it replies `+PONG`; with one, it echoes the message back
/// as a bulk string instead.
#[derive(Debug)]
pub struct PingCommand {
    message: Option<String>,
}

impl PingCommand {
    /// Build a `PingCommand` from the arguments that followed `PING` in the
    /// command array (i.e. everything except the command name itself).
    pub fn from_arguments(arguments: Vec<RespType>) -> Result<PingCommand, CommandError> {
        let mut arguments = arguments.into_iter();

        let message = match arguments.next() {
            None => None,
            Some(RespType::BulkString(message)) => Some(message),
            Some(_) => {
                return Err(CommandError::InvalidArguments(
                    "ERR PING message must be a bulk string".to_string(),
                ));
            }
        };

        if arguments.next().is_some() {
            return Err(CommandError::InvalidArguments(
                "ERR wrong number of arguments for 'ping' command".to_string(),
            ));
        }

        Ok(PingCommand { message })
    }

    /// Produce the RESP reply for this `PING`.
    pub fn execute(self) -> RespType {
        match self.message {
            Some(message) => RespType::BulkString(message),
            None => RespType::SimpleString("PONG".to_string()),
        }
    }
}
