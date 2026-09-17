use anyhow::{Error, Result};
use futures::{SinkExt, StreamExt};
use log::{error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

mod resp;
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

    pub async fn accept_conn(&mut self) -> Result<TcpStream> {
        match self.listener.accept().await {
            Ok((socket, _)) => Ok(socket),
            Err(e) => Err(Error::from(e)),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let sock = match self.accept_conn().await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("{}", e);
                    panic!("Error accepting connection")
                }
            };

            tokio::spawn(handle_connection(sock));
        }
    }
}

/// Drives a single connection: frames RESP values off the socket via
/// `RespCodec` and echoes each one back, until the client disconnects
/// or sends something the codec can't parse.
async fn handle_connection(sock: TcpStream) {
    let mut frames = Framed::new(sock, RespCodec);

    while let Some(result) = frames.next().await {
        let response = match result {
            Ok(value) => value,
            Err(e) => {
                error!("Error reading request: {}", e);
                RespType::SimpleError(e.to_string())
            }
        };

        if let Err(e) = frames.send(response).await {
            error!("Error writing response: {}", e);
            break;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let addr = "127.0.0.1:6300".to_string();

    let listener = match TcpListener::bind(&addr).await {
        Ok(tcp_listener) => {
            info!("TCP listener start at addr: {}", addr);
            tcp_listener
        }
        Err(e) => panic!(
            "Couldn't bind the TCP listener to addr: {}, error: {}",
            addr, e
        ),
    };

    let mut server = Server::new(listener);
    server.run().await?;
    Ok(())
}
