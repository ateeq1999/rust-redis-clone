use anyhow::{Error, Result};
use bytes::BytesMut;
use log::{error, info};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

mod resp;
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
            let mut sock = match self.accept_conn().await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("{}", e);
                    panic!("Error accepting connection")
                }
            };

            tokio::spawn(async move {
                let mut buffer = BytesMut::with_capacity(512);
                if let Err(e) = sock.read_buf(&mut buffer).await {
                    error!("Error reading request: {}", e);
                    panic!("Error reading request: {}", e);
                }

                let resp_data = match RespType::parse(&buffer) {
                    Ok((data, _)) => data,
                    Err(e) => RespType::SimpleError(e.to_string()),
                };

                if let Err(e) = sock.write_all(&resp_data.to_bytes()[..]).await {
                    error!("{}", e);
                    panic!("Error writing response")
                }
            });
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
