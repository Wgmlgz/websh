use crate::port::handle_port;
use crate::shell::{handle_pty, Session, SessionMap};
use crate::signal::Signaling;
use crate::state::State;
use anyhow::{anyhow, Ok, Result};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::sync::{broadcast, Mutex};
use ts_rs::TS;
use uuid::Uuid;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct Peer {
    pub peer_connection: Arc<RTCPeerConnection>, // To signal done
}

pub type PeerMap = Arc<Mutex<HashMap<String, Peer>>>;

#[derive(Default, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DataChannelSettingsMsg {
    variant: String,
    session_id: Option<String>,
}

impl<T> State<T>
where
    // i have a nuke in my pickup truck, how the fuck is this even works?
    T: Signaling + std::marker::Send + std::marker::Sync + 'static,
{
    // Session does not exist, create new Session
    pub fn create_session(
        self: Arc<Self>,
        pc: Arc<RTCPeerConnection>,
        variant: String,
        mut peer_done_rx: broadcast::Receiver<()>,
    ) -> Result<Session> {
        let (to_pty_tx, to_pty_rx) = mpsc::channel::<Bytes>(100);
        let (from_pty_tx, _) = broadcast::channel::<Bytes>(100);
        let (done_tx, done_rx) = broadcast::channel::<()>(1);

        // send done if peer is done
        let done_tx_copy = done_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = peer_done_rx
                .recv()
                .await
                .map_err(|e| anyhow!(e))
                .and_then(|_| done_tx_copy.send(()).map_err(|e| anyhow!(e)))
            {
                log::error!("Some error idc 2: {}", e);
            }
        });

        let self_clone = self.clone();
        // Start PTY handler
        tokio::spawn({
            let from_pty_tx = from_pty_tx.clone();
            let self_clone = self_clone.clone();
            async move {
                match variant.as_str() {
                    "control" => {
                        self_clone
                            .handle_control(pc, from_pty_tx, to_pty_rx, done_rx)
                            .await
                    }
                    "web_shell" => handle_pty(from_pty_tx, to_pty_rx, done_rx).await,
                    "port" => handle_port(from_pty_tx, to_pty_rx, done_rx).await,
                    _ => log::error!("unknown data channel"),
                }
            }
        });

        let session = Session {
            to_pty: to_pty_tx.clone(),
            from_pty: from_pty_tx.clone(),
            done_tx: done_tx.clone(),
        };
        Ok(session)
    }

    pub fn on_data_channel(
        self: Arc<Self>,
        pc: Arc<RTCPeerConnection>,
        d: Arc<RTCDataChannel>,
        session_map: SessionMap,
        mut peer_done_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        let d_label = d.label().to_owned();
        let d_id = d.id();
        if d_label == "dummy" {
            return Ok(());
        }

        let msg: DataChannelSettingsMsg = serde_json::from_str(d_label.as_str()).unwrap();

        let variant = msg.variant;
        let session_id = match msg.session_id {
            Some(id) => id,
            None => Uuid::new_v4().to_string(),
        };

        log::info!("New DataChannel {variant} {d_id}");

        // Check if session already exists
        let session = {
            let mut map = session_map.lock().unwrap();
            if variant == "port" {
                let session = self.create_session(pc, variant, peer_done_rx.resubscribe())?;
                map.insert(session_id.clone(), session.clone());
                session
            } else {
                if let Some(pty_session) = map.get(&session_id) {
                    pty_session.clone()
                } else {
                    let session = self.create_session(pc, variant, peer_done_rx.resubscribe())?;
                    map.insert(session_id.clone(), session.clone());
                    session
                }
            }
        };

        let session_done_rx_copy = session.done_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = peer_done_rx
                .recv()
                .await
                .map_err(|e| anyhow!(e))
                .and_then(|_| session_done_rx_copy.send(()).map_err(|e| anyhow!(e)))
            {
                log::error!("Some error idc 3: {}", e);
            }
        });
        // Register channel opening handling
        let d2 = Arc::clone(&d);
        // let d_label2 = d_label.clone();
        // let d_id2 = d_id;
        d.on_close(Box::new(move || {
            log::info!("Data channel closed");
            if session.done_tx.is_empty() {
                let _ = session.done_tx.send(());
            }
            Box::pin(async {})
        }));

        // Now we have the PTYSession
        // Subscribe to the broadcast channel to receive data from PTY
        let mut from_pty_rx = session.from_pty.subscribe();

        // Clone the sender to send data to PTY
        let to_pty = session.to_pty.clone();

        d.on_open(Box::new(move || {
            let d_clone = Arc::clone(&d2); // Clone the Arc to use in the async block
            Box::pin(async move {
                // Launch a task to handle sending messages received via the channel
                tokio::spawn(async move {
                    while let Result::Ok(message) = from_pty_rx.recv().await {
                        if d_clone.send(&message).await.is_err() {
                            log::error!("Failed to send message over data channel");
                            break;
                        }
                    }
                });
            })
        }));

        // Register text message handling
        d.on_message(Box::new(move |msg: DataChannelMessage| {
            let ptx_clone = to_pty.clone(); // Clone the sender for use in the async context
                                            // let msg_str = String::from_utf8(msg.data.to_vec()).unwrap(); // Convert the received message to a String

            Box::pin(async move {
                // Send the message to the PTY task asynchronously
                if let Err(e) = ptx_clone.send(msg.data).await {
                    log::error!("Failed to send message to PTY: {}", e);
                }
            })
        }));
        Ok(())
    }
}
