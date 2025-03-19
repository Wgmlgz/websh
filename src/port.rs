use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;

use tokio::sync::{broadcast, mpsc};

pub async fn handle_port(
    tx: broadcast::Sender<Bytes>,  // From server to clients
    mut rx: mpsc::Receiver<Bytes>, // From clients to server
    _done_rx: broadcast::Receiver<()>,
) {
    const SSH_SERVER_HOST: &str = "localhost";
    const SSH_SERVER_PORT: u16 = 22;

    // Create a TCP connection to the SSH server
    let ssh_addr = format!("{}:{}", SSH_SERVER_HOST, SSH_SERVER_PORT);
    let tcp_stream = match TcpStream::connect(&ssh_addr).await {
        Ok(s) => {
            log::info!("Connected to SSH server");
            s
        }
        Err(e) => {
            log::error!("TCP connect error: {}", e);
            return;
        }
    };

    let (mut tcp_reader, mut tcp_writer) = tcp_stream.into_split();

    let ws_to_tcp = async {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = tcp_writer.write_all(&msg).await {
                log::error!("TCP write error: {}", e);
                break;
            }
        }
        tcp_writer.shutdown().await.ok();
        log::info!("WebSocket client disconnected");
    };

    // Handle TCP socket data and send it over the WebSocket
    let tcp_to_ws = async {
        let mut buffer = [0u8; 1024];
        loop {
            match tcp_reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    let data = Bytes::copy_from_slice(&buffer[..n]);
                    if let Err(e) = tx.send(data) {
                        log::error!("WebSocket send error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    log::error!("TCP read error: {}", e);
                    break;
                }
            }
        }
        // tx.close().await.ok();
        log::info!("Disconnected from SSH server");
    };

    tokio::select! {
        _ = ws_to_tcp => (),
        _ = tcp_to_ws => (),
    }

    log::info!("handle_port exiting");
}
