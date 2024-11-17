use std::sync::Arc;

use anyhow::{Ok, Result};

use bytes::Bytes;
use clap::Parser;
use env_logger::Env;
use futures_util::StreamExt;
use rand::distributions::{Alphanumeric, DistString};
use signal::State;
use tokio::{
    io::{self, AsyncReadExt},
    net::{TcpListener, TcpStream},
};
use tokio_tungstenite::tungstenite;

pub mod peer;
pub mod port;
pub mod shell;
pub mod signal;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    name: Option<String>,
    url: Option<String>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

pub async fn start_client(cli: Cli) -> Result<()> {
    let cli = Arc::new(cli);

    // Define the port to listen on
    let addr = "127.0.0.1:2222"; // Adjust as needed

    let listener = TcpListener::bind(addr).await?;

    log::info!("TCP server listening on {}", addr);

    while let io::Result::Ok((tcp_stream, addr)) = listener.accept().await {
        let cli = cli.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(cli, tcp_stream).await {
                log::error!("Error while handle_client {}", e.to_string());
            }
        });
    }
    // tokio::select! {


    // };
    // while let Some(msg) = state.read.next().await {

    // }
    // });
    // Shared list of connected clients
    // let clients = Arc::new(Mutex::new(Vec::new()));

    // Task to read from rx and write to all connected clients
    // let clients_for_rx = clients.clone();
    // let rx_task = tokio::spawn(async move {
    //     while let Some(msg) = rx.recv().await {
    //         let clients = clients_for_rx.lock().unwrap().clone();
    //         for mut client in clients {
    //             let msg = msg.clone();
    //             tokio::spawn(async move {
    //                 if let Err(e) = client.write_all(&msg).await {
    //                     log::error!("Failed to write to TCP client: {}", e);
    //                 }
    //             });
    //         }
    //     }
    // });

    // loop {
    //     match listener.accept().await {
    //         io::Result::Ok((tcp_stream, addr)) => {

    //         }
    //         Err(e) => {}
    //     }
    //     // Ok((tcp_stream, addr)) = listener.accept() => {
    //     //     log::info!("New TCP client connected: {}", addr);

    //     //     let tx_clone = tx.clone();
    //     //     let clients_clone = clients.clone();

    //     //     // Add the new client to the list
    //     //     {
    //     //         let mut clients_lock = clients_clone.lock().unwrap();
    //     //         clients_lock.push(tcp_stream);
    //     //     }

    //     //     // Handle the client in a separate task
    //     //     tokio::spawn(async move {
    //     //         handle_client(tcp_stream, tx_clone, clients_clone).await;
    //     //     });
    //     // },
    // }

    // Clean up
    log::info!("Shutting down TCP server");
    // let _ = rx_task.await;
    Ok(())
}

async fn handle_client(
    cli: Arc<Cli>,
    mut tcp_stream: TcpStream,
    // tx: broadcast::Sender<Bytes>,
    // clients: Arc<Mutex<Vec<TcpStream>>>,
) -> Result<()> {


    // let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    // let session = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    // log::info!("Staring with {}", &name);

    // let name = cli.name.clone().unwrap_or(name.clone());
    // let url = cli
    //     .url
    //     .clone()
    //     .unwrap_or("wss://websh.amogos.pro/signaling".into());

    // let mut state = State::new(name, "client".to_string(), url).await?;

    // tokio::spawn(async move {
    //     while let Some(msg) = state.read.next().await {
    //         match msg {
    //             Result::Ok(tungstenite::Message::Text(text)) => {
    //                 if let Err(e) = state.handle_ws_message(text).await {
    //                     log::error!("Error while handling websocket message {}", e.to_string());
    //                 }
    //             }
    //             Result::Err(e) => {
    //                 log::error!("Error with websocket message {}", e.to_string());
    //             }
    //             _ => {
    //                 log::warn!("Websocket message type is not supported ");
    //             }
    //         }
    //     }
    // });
    // state.create_offer("server1".into(), session).await?;

    // tokio::spawn(async move {
    //     log::info!("Starting app");
    //     if let Err(e) = signal::connect(name, "client".into(), url).await {
    //         log::error!("Error while running app: {}", e.to_string());
    //     };
    // });

    let mut buf = [0u8; 1024];

    loop {
        let n = match tcp_stream.read(&mut buf).await {
            io::Result::Ok(0) => {
                // Client disconnected
                log::info!("Client disconnected");
                break;
            }
            io::Result::Ok(n) => n,
            Err(e) => {
                log::error!("Failed to read from client: {}", e);
                break;
            }
        };

        let msg = Bytes::copy_from_slice(&buf[..n]);

        // Broadcast the message to other clients
        // if let Err(e) = tx.send(msg) {
        //     log::error!("Failed to send message to clients: {}", e);
        // }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    start_client(cli).await;

    Ok(())
}
