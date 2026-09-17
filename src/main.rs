use anyhow::{Error, Result};
use futures::{SinkExt, StreamExt};
use log::{error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

mod command;
mod resp;
use crate::command::Command;
use crate::resp::codec::RespCodec;
use crate::resp::types::RespType;

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn new(listener: TcpListener) -> Server {
        Server { listener }
    }

    pub async fn accept_connection(&mut self) -> Result<TcpStream> {
        match self.listener.accept().await {
            Ok((connection, _)) => Ok(connection),
            Err(accept_error) => Err(Error::from(accept_error)),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let connection = match self.accept_connection().await {
                Ok(connection) => connection,
                Err(accept_error) => {
                    error!("{}", accept_error);
                    panic!("Error accepting connection")
                }
            };

            tokio::spawn(handle_connection(connection));
        }
    }
}

/// Drives a single connection: frames RESP values off the socket via
/// `RespCodec`, dispatches each one as a command, and writes back the
/// command's response - until the client disconnects or sends something
/// the codec can't parse.
async fn handle_connection(connection: TcpStream) {
    let mut frames = Framed::new(connection, RespCodec);

    while let Some(decoded_frame) = frames.next().await {
        let response = match decoded_frame {
            // A real Redis client always sends commands as an array of bulk
            // strings, so this is the only shape we dispatch.
            Ok(RespType::Array(elements)) => match Command::from_array_elements(elements) {
                Ok(command) => command.execute(),
                Err(command_error) => RespType::from(command_error),
            },
            Ok(_) => RespType::SimpleError(
                "ERR Protocol error: expected a command array".to_string(),
            ),
            Err(read_error) => {
                error!("Error reading request: {}", read_error);
                RespType::SimpleError(read_error.to_string())
            }
        };

        if let Err(write_error) = frames.send(response).await {
            error!("Error writing response: {}", write_error);
            break;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let server_address = "127.0.0.1:6300".to_string();

    let listener = match TcpListener::bind(&server_address).await {
        Ok(tcp_listener) => {
            info!("TCP listener start at addr: {}", server_address);
            tcp_listener
        }
        Err(bind_error) => panic!(
            "Couldn't bind the TCP listener to addr: {}, error: {}",
            server_address, bind_error
        ),
    };

    let mut server = Server::new(listener);
    server.run().await?;
    Ok(())
}
