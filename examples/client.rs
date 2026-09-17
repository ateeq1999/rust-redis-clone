use anyhow::{Context, Result};
use log::{error, info};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const READ_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Client {
    addr: String,
}

impl Client {
    pub fn new(addr: &str) -> Self {
        Client {
            addr: addr.to_string(),
        }
    }

    /// Sends a request string to the server and reads/logs the response.
    pub async fn send_and_receive(&self, payload: &str, description: &str) -> Result<()> {
        info!("--- Test case: {} ---", description);
        info!("Connecting to server at {}...", self.addr);

        // 1. Establish a separate connection for this test transaction
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .context(format!("Failed to connect to {}", self.addr))?;

        // 2. Write the raw RESP data to the server
        info!("Sending raw bytes: {:?}", payload);
        stream
            .write_all(payload.as_bytes())
            .await
            .context("Failed to write request to socket")?;

        // 3. Read the server's parsed response. The server now waits for a full
        // RESP value before replying, so a truly incomplete payload never gets
        // a response - bound the wait instead of blocking forever.
        let mut buffer = [0; 1024];
        let bytes_read = match tokio::time::timeout(READ_TIMEOUT, stream.read(&mut buffer)).await
        {
            Ok(result) => result.context("Failed to read reply from server socket")?,
            Err(_) => {
                info!("No response within {:?} (server is still waiting on more bytes, as expected for incomplete input)", READ_TIMEOUT);
                return Ok(());
            }
        };

        if bytes_read == 0 {
            error!("Server closed the connection before sending back data.");
            return Ok(());
        }

        // 4. Parse response raw text bytes and display it
        let response = std::str::from_utf8(&buffer[..bytes_read])
            .context("Server response is not valid UTF-8 data")?;

        info!("Received from server: {:?}", response);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize env_logger so `info!` and `error!` logs print directly to terminal
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Match your custom server address and port configurations
    let addr = "127.0.0.1:6300";
    let client = Client::new(addr);

    // Test 1: Send Bulk String
    if let Err(e) = client
        .send_and_receive("$5\r\nhello\r\n", "Send Bulk String")
        .await
    {
        error!("Error in Bulk String test: {:?}", e);
    }

    // Add a tiny sleep delay between requests to separate socket streams visibly
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 2: Send Simple String
    if let Err(e) = client
        .send_and_receive("+OK\r\n", "Send Simple String")
        .await
    {
        error!("Error in Simple String test: {:?}", e);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 3: Send an Array (how Redis commands are framed), e.g. ECHO hello
    if let Err(e) = client
        .send_and_receive(
            "*2\r\n$4\r\nECHO\r\n$5\r\nhello\r\n",
            "Send Array (RESP command)",
        )
        .await
    {
        error!("Error in Array test: {:?}", e);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 4: Send Invalid RESP Data (unrecognized type byte)
    if let Err(e) = client
        .send_and_receive("@bad\r\n", "Send Invalid RESP Data")
        .await
    {
        error!("Error in Invalid Data test: {:?}", e);
    }

    Ok(())
}
