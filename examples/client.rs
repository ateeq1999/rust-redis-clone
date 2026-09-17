use anyhow::{Context, Result};
use log::{error, info};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// Encodes a command and its arguments as a RESP array of bulk strings - the
/// wire format every real Redis client sends. For example,
/// `encode_command(&["SET", "foo", "bar"])` produces
/// `"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"`, without having to count
/// bytes and build that string by hand.
fn encode_command(parts: &[&str]) -> String {
    let mut encoded = format!("*{}\r\n", parts.len());
    for part in parts {
        // RESP bulk string lengths are byte counts, not character counts -
        // `str::len()` gives bytes, which is what we want here.
        encoded.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }
    encoded
}

pub struct Client {
    server_address: String,
}

impl Client {
    pub fn new(server_address: &str) -> Self {
        Client {
            server_address: server_address.to_string(),
        }
    }

    /// Sends a request string to the server and reads/logs the response.
    pub async fn send_and_receive(&self, payload: &str, description: &str) -> Result<()> {
        info!("--- Test case: {} ---", description);
        info!("Connecting to server at {}...", self.server_address);

        // 1. Establish a separate connection for this test transaction
        let mut stream = TcpStream::connect(&self.server_address)
            .await
            .context(format!("Failed to connect to {}", self.server_address))?;

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
            Ok(read_result) => read_result.context("Failed to read reply from server socket")?,
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
    let server_address = "127.0.0.1:6300";
    let client = Client::new(server_address);

    // Test 1: PING with no arguments -> +PONG
    if let Err(test_error) = client
        .send_and_receive(&encode_command(&["PING"]), "PING (no arguments)")
        .await
    {
        error!("Error in PING test: {:?}", test_error);
    }

    // Add a tiny sleep delay between requests to separate socket streams visibly
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 2: PING with a message -> echoed back as a bulk string
    if let Err(test_error) = client
        .send_and_receive(
            &encode_command(&["PING", "hello"]),
            "PING (with a message)",
        )
        .await
    {
        error!("Error in PING-with-message test: {:?}", test_error);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 3: SET foo bar -> +OK. Each test opens a fresh connection, but the
    // store lives on the server and is shared across all of them, so the
    // GETs below (on their own connections) will still see this value.
    if let Err(test_error) = client
        .send_and_receive(&encode_command(&["SET", "foo", "bar"]), "SET foo bar")
        .await
    {
        error!("Error in SET test: {:?}", test_error);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 4: GET foo -> the bulk string "bar", set by the previous test
    if let Err(test_error) = client
        .send_and_receive(&encode_command(&["GET", "foo"]), "GET foo")
        .await
    {
        error!("Error in GET test: {:?}", test_error);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 5: GET on a key that was never set -> a null bulk string ($-1)
    if let Err(test_error) = client
        .send_and_receive(&encode_command(&["GET", "missing"]), "GET on a missing key")
        .await
    {
        error!("Error in GET-missing-key test: {:?}", test_error);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 6: An unrecognized command name -> a SimpleError from the command layer
    if let Err(test_error) = client
        .send_and_receive(&encode_command(&["FOOBAR"]), "Unknown command")
        .await
    {
        error!("Error in Unknown Command test: {:?}", test_error);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 7: A well-formed RESP value that isn't a command array -> protocol error.
    // Real Redis clients always send commands as arrays of bulk strings.
    if let Err(test_error) = client
        .send_and_receive("+OK\r\n", "Non-array top-level value")
        .await
    {
        error!("Error in Non-Array test: {:?}", test_error);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 8: Send Invalid RESP Data (unrecognized type byte)
    if let Err(test_error) = client
        .send_and_receive("@bad\r\n", "Send Invalid RESP Data")
        .await
    {
        error!("Error in Invalid Data test: {:?}", test_error);
    }

    Ok(())
}
