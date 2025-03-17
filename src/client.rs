use std::sync::Arc;

use anyhow::{Ok, Result};

use bytes::Bytes;
use clap::Parser;
use env_logger::Env;
use peer::Peer;
use rand::distributions::{Alphanumeric, DistString};
use signal::{Message, Signaling};
use state::State;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use webrtc::{
    data_channel::data_channel_message::DataChannelMessage, peer_connection::RTCPeerConnection,
};

pub mod peer;
pub mod port;
pub mod shell;
pub mod signal;
pub mod recording;
pub mod control;
pub mod state;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    name: Option<String>,
    url: Option<String>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

pub async fn start_client(cli: Cli) -> Result<()> {
    let cli = Arc::new(cli);

    let peer_connection = connect_to_peer(cli).await?;
    // Define the port to listen on
    let addr = "127.0.0.1:2222"; // Adjust as needed

    let listener = TcpListener::bind(addr).await?;

    log::info!("TCP server listening on {}", addr);

    while let io::Result::Ok((tcp_stream, _)) = listener.accept().await {
        let peer_connection2 = peer_connection.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(peer_connection2, tcp_stream).await {
                log::error!("Error while handle_client {}", e.to_string());
            }
        });
    }

    log::info!("Shutting down TCP server");
    Ok(())
}

async fn connect_to_peer(cli: Arc<Cli>) -> Result<Arc<RTCPeerConnection>> {
    log::info!("new connection");
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    log::info!("Staring with {}", &name);

    let name = cli.name.clone().unwrap_or(name.clone());
    let url = cli
        .url
        .clone()
        .unwrap_or("wss://websh.amogos.pro/signaling".into());

    let state = State::new(name.clone(), "client".to_string(), url).await?;
    let state = Arc::new(state);

    let state_clone = state.clone();
    tokio::spawn(async move {
        state_clone.signal_loop().await;
        Box::pin(async move {})
    });
    let target: String = "server1".into();

    let (peer_connection, _done_rx) = state.create_peer_connection(target.clone()).await?;

    {
        let user_name = target.clone();
        let mut map = state.peer_map.lock().await;
        if let Some(_) = map.get(&user_name) {
            log::error!("Peer exists");
            // Session exists
        } else {
            map.insert(
                user_name,
                Peer {
                    peer_connection: peer_connection.clone(),
                },
            );
        }
    }

    let peer_connection2 = peer_connection.clone();
    let name2 = name.clone();
    let target2 = target.clone();
    let signaling2 = state.signaling.clone();

    peer_connection.on_negotiation_needed(Box::new(move || {
        let peer_connection2 = peer_connection2.clone();

        let name = name2.clone();
        let target = target2.clone();
        let signaling = signaling2.clone();

        Box::pin(async move {
            let local_desc = peer_connection2.create_offer(None).await.unwrap();
            peer_connection2
                .set_local_description(local_desc.clone())
                .await
                .unwrap();

            let signal_msg = Message {
                r#type: "offer".to_string(),
                name: Some(name.clone().to_string()),
                target: Some(target.clone()),
                data: Some(serde_json::to_string(&local_desc).unwrap()),
                peer_type: None,
                from: None,
            };

            signaling.send(serde_json::to_string(&signal_msg).unwrap());
        })
    }));

    let signal_msg = Message {
        r#type: "connect".to_string(),
        name: None,
        target: Some(target.clone()),
        data: None,
        peer_type: None,
        from: None,
    };

    state
        .signaling
        .send(serde_json::to_string(&signal_msg).unwrap());

    // create dummy data channel to force on_negotiation_needed
    let _ = peer_connection.create_data_channel("dummy", None).await?;

    Ok(peer_connection)
}

async fn handle_client(
    peer_connection: Arc<RTCPeerConnection>,
    tcp_stream: TcpStream,
) -> Result<()> {
    let d = peer_connection.create_data_channel("port", None).await?;

    let (to_pty_tx, mut to_pty_rx) = mpsc::channel::<Bytes>(100);

    let (done_tx, mut done_rx) = mpsc::channel::<()>(1);

    d.on_message(Box::new(move |msg: DataChannelMessage| {
        let ptx_clone = to_pty_tx.clone();

        Box::pin(async move {
            // Send the message to the PTY task asynchronously
            if let Err(e) = ptx_clone.send(msg.data).await {
                log::error!("Failed to send message to PTY: {}", e);
            }
        })
    }));

    let d2 = d.clone();

    d.on_open(Box::new(move || {
        let d_clone = Arc::clone(&d2); // Clone the Arc to use in the async block
        Box::pin(async move {
            let (mut tcp_reader, mut tcp_writer) = tcp_stream.into_split();

            let tcp_to_ws = async {
                let mut buffer = [0u8; 1024];
                loop {
                    match tcp_reader.read(&mut buffer).await {
                        Result::Ok(0) => break,
                        Result::Ok(n) => {
                            if let Err(e) =
                                d_clone.send(&Bytes::copy_from_slice(&buffer[..n])).await
                            {
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
                log::info!("TCP client disconnected");
            };

            let ws_to_tcp = async {
                while let Some(msg) = to_pty_rx.recv().await {
                    if let Err(e) = tcp_writer.write_all(&msg).await {
                        log::error!("TCP write error: {}", e);
                        break;
                    }
                }
                tcp_writer.shutdown().await.ok();
                log::info!("WebSocket connection closed");
            };

            tokio::select! {
                _ = tcp_to_ws => (),
                _ = ws_to_tcp => (),
            }
            done_tx.send(()).await.unwrap();
        })
    }));

    done_rx.recv().await;

    log::info!("connection end");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    if let Err(e) = start_client(cli).await {
        log::error!("Error while handling {}", e.to_string())
    }

    Ok(())
}
