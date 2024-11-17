use bytes::Bytes;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::thread;
use tokio::sync::mpsc::Receiver;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
pub struct PTYSession {
    pub to_pty: mpsc::Sender<String>,        // To send data to PTY
    pub from_pty: broadcast::Sender<String>, // To receive data from PTY
    pub done_tx: mpsc::Sender<()>,           // To signal done
}

pub type SessionMap = Arc<Mutex<HashMap<String, PTYSession>>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct ShellMsg {
    pub resize: Option<PtySize>,
    pub input: Option<String>,
    pub output: Option<String>,
}

pub async fn handle_port(
    tx: broadcast::Sender<Bytes>,  // From server to clients
    mut rx: mpsc::Receiver<Bytes>, // From clients to server
    mut done_rx: Receiver<()>,
) {
    let addr = "127.0.0.1:22"; // SSH server address
    let stream = match tokio::net::TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to connect to {}: {}", addr, e);
            return;
        }
    };

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut reader, mut writer) = stream.into_split();

    // Task to read from TCP stream and send to tx
    let tx_clone = tx.clone();
    let mut reader_task = tokio::spawn(async move {
        loop {
            let mut buf = [0u8; 1024];
            let n = match reader.read(&mut buf).await {
                Ok(n) if n == 0 => {
                    // Connection closed
                    log::info!("TCP connection closed by remote");
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    log::error!("Failed to read from TCP stream: {}", e);
                    break;
                }
            };

            let msg = Bytes::copy_from_slice(&buf[..n]);

            // Send message to clients
            match tx_clone.send(msg) {
                Ok(_) => (),
                Err(e) => {
                    log::error!("Failed to send message to clients: {}", e);
                    break;
                }
            }
        }
    });

    // Task to read from rx and write to TCP stream
    let mut writer_task = tokio::spawn(async move {
        while let Some(input) = rx.recv().await {
            if let Err(e) = writer.write_all(&input).await {
                log::error!("Failed to write to TCP stream: {}", e);
                break;
            }
        }
    });

    // Wait for the done signal or tasks to finish
    tokio::select! {
        _ = done_rx.recv() => {
            log::info!("Received done signal");
        },
        _ = &mut reader_task => {
            log::info!("Reader task finished");
        },
        _ = &mut writer_task => {
            log::info!("Writer task finished");
        },
    };

    // Clean up
    // Close the connection
    // let _ = writer.shutdown().await;

    // Ensure tasks have finished
    let _ = reader_task.await;
    let _ = writer_task.await;

    log::info!("handle_port exiting");
}
