use crate::port::handle_port;
use crate::shell::{handle_pty, PTYSession, SessionMap};
use anyhow::{anyhow, Ok, Result};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct Peer {
    pub peer_connection: Arc<RTCPeerConnection>, // To signal done
}

pub type PeerMap = Arc<Mutex<HashMap<String, Peer>>>;

pub fn on_data_channel(
    d: Arc<RTCDataChannel>,
    session: Option<String>,
    session_map: SessionMap,
) -> Result<()> {
    let session_id = session.unwrap_or_else(|| "default".to_string());

    let d_label = d.label().to_owned();
    let d_id = d.id();
    log::info!("New DataChannel {d_label} {d_id}");

    // Check if session already exists
    let pty_session = {
        let mut map = session_map.lock().unwrap();
        if let Some(pty_session) = map.get(&session_id) {
            // Session exists
            pty_session.clone()
        } else {
            // Session does not exist, create new PTYSession
            let (to_pty_tx, to_pty_rx) = mpsc::channel::<Bytes>(100);
            let (from_pty_tx, _) = broadcast::channel::<Bytes>(100);
            let (done_tx, done_rx) = mpsc::channel::<()>(1);

            // Start PTY handler
            tokio::spawn({
                let from_pty_tx = from_pty_tx.clone();
                async move {
                    match d_label.as_str() {
                        "web_shell" => handle_pty(from_pty_tx, to_pty_rx, done_rx).await,
                        "port" => handle_port(from_pty_tx, to_pty_rx, done_rx).await,
                        _ => log::error!("unknown data channel"),
                    }
                }
            });

            let pty_session = PTYSession {
                to_pty: to_pty_tx.clone(),
                from_pty: from_pty_tx.clone(),
                done_tx: done_tx.clone(),
            };

            // Store PTYSession in map
            map.insert(session_id.clone(), pty_session.clone());

            pty_session
        }
    };

    // Register channel opening handling
    let d2 = Arc::clone(&d);
    // let d_label2 = d_label.clone();
    // let d_id2 = d_id;
    d.on_close(Box::new(move || {
        log::info!("Data channel closed");
        let _ = pty_session.done_tx.try_send(());
        Box::pin(async {})
    }));

    // Now we have the PTYSession
    // Subscribe to the broadcast channel to receive data from PTY
    let mut from_pty_rx = pty_session.from_pty.subscribe();

    // Clone the sender to send data to PTY
    let to_pty = pty_session.to_pty.clone();

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
