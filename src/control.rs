use std::default;
use std::sync::Arc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;

use tokio::sync::{broadcast, mpsc};
use ts_rs::TS;
use webrtc::peer_connection::RTCPeerConnection;

use crate::recording::add_video;
use crate::signal::Signaling;
use crate::state::State;

#[derive(Default, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ControlMsg {
    #[default]
    Empty,
    StartVideo(StartVideoMsg),
}

#[derive(Default, Clone, Serialize, Deserialize, TS)]
pub struct StartVideoMsg {
    display: i32,
}

impl<T> State<T>
where
    T: Signaling,
{
    pub async fn handle_control(
        self: Arc<Self>,
        pc: Arc<RTCPeerConnection>,
        tx: broadcast::Sender<Bytes>,  // From server to clients
        mut rx: mpsc::Receiver<Bytes>, // From clients to server
        mut done_rx: Receiver<()>,
    ) {
        // Asynchronously receive messages and write to PTY
        tokio::spawn(async move {
            while let Some(json) = rx.recv().await {
                let sus = json.to_vec();
                let sus = sus.as_slice();
                let msg: ControlMsg =
                    serde_json::from_str(String::from_utf8_lossy(sus).to_string().as_str())
                        .unwrap();

                match msg {
                    ControlMsg::Empty => todo!(),
                    ControlMsg::StartVideo(start_video_msg) => {
                        let p = pc.clone();
                        tokio::spawn(async move {
                            add_video(&p, start_video_msg).await.unwrap();
                        });
                    }
                }
                let json = serde_json::to_string(&ControlMsg::default()).unwrap();

                // Use blocking send to the async channel (note this can block the thread if the channel is full)
                match tx.send(json.into()) {
                    Ok(_) => (),
                    Err(e) => {
                        log::error!("Failed to send message to async context: {}", e);
                        break;
                    }
                }
            }
        });

        tokio::select! {
            _ = done_rx.recv() => {
                log::info!("Received done signal");
            }
        };
    }
}
