use crate::shell::{handle_pty, PTYSession, SessionMap};
use anyhow::{anyhow, Error, Ok, Result};
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

use crate::peer::{on_data_channel, Peer, PeerMap};

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    r#type: String,
    name: Option<String>,
    target: Option<String>,
    session: Option<String>,
    data: Option<String>,
    peer_type: Option<String>,
    from: Option<String>,
}

struct State {
    pub api: API,
    pub config: RTCConfiguration,
    pub session_map: SessionMap,
    pub my_name: String,
    pub write: Arc<
        Mutex<
            futures_util::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tungstenite::Message,
            >,
        >,
    >,
    pub read: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    pub peer_map: PeerMap,
}

impl State {
    async fn handle_signal(&self, message: Message) -> Result<()> {
        // Handle signaling messages from the user
        let data = message.clone().data.ok_or(anyhow!("No data in message"))?;
        let sdp: RTCSessionDescription = serde_json::from_str::<RTCSessionDescription>(&data)?;

        // Create a new RTCPeerConnection
        let peer_connection = Arc::new(self.api.new_peer_connection(self.config.clone()).await?);

        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Set the handler for Peer connection state
        // This will notify you when the peer has connected/disconnected
        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                log::info!("Peer Connection State has changed: {s}");

                if s == RTCPeerConnectionState::Failed {
                    // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
                    // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
                    // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
                    log::error!("Peer Connection has gone to failed exiting");
                    let _ = done_tx.try_send(());
                }

                Box::pin(async {})
            },
        ));

        let message_clone = message.clone();
        let session_map = self.session_map.clone();
        // Register data channel creation handling
        peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
            if let Err(e) = on_data_channel(d, message_clone.clone().session, session_map.clone()) {
                log::error!("Failed to handle data channel: {}", e.to_string())
            }
            Box::pin(async {})
        }));

        peer_connection.set_remote_description(sdp).await?;

        let my_name = self.my_name.clone();

        if peer_connection
            .remote_description()
            .await
            .ok_or(anyhow!("No remote description"))?
            .sdp_type
            == RTCSdpType::Offer
        {
            let answer = peer_connection.create_answer(None).await?;
            peer_connection.set_local_description(answer).await?;
            let local_desc = peer_connection
                .local_description()
                .await
                .ok_or(anyhow!("Can't get local description"))?;

            let signal_msg = Message {
                r#type: "signal".to_string(),
                name: Some(my_name.clone().to_string()),
                target: Some(message.clone().from.ok_or(anyhow!("No from provides"))?),
                data: Some(serde_json::to_string(&local_desc)?),
                session: None,
                peer_type: None,
                from: None,
            };
            self.write
                .lock()
                .await
                .send(tungstenite::Message::Text(serde_json::to_string(
                    &signal_msg,
                )?))
                .await?;
        }
        let write = self.write.clone();

        let message_clone = message.clone();
        peer_connection.on_ice_candidate(Box::new(move |candidate| {
            let from = message_clone.clone().from;
            let my_name = my_name.clone();
            if let Some(candidate) = candidate {
                let write_clone = write.clone();
                let signal_msg = Message {
                    r#type: "candidate".to_string(),
                    name: Some(my_name.to_string()),
                    target: Some(from.clone().unwrap()),
                    data: Some(serde_json::to_string(&candidate.to_json().unwrap()).unwrap()),
                    session: None,
                    peer_type: None,
                    from: None,
                };
                tokio::spawn(async move {
                    let mut write_guard = write_clone.lock().await;

                    write_guard
                        .send(tungstenite::Message::Text(
                            serde_json::to_string(&signal_msg).unwrap(),
                        ))
                        .await
                        .unwrap();
                });
            }

            Box::pin(async {})
        }));
        {
            let user_name = message.clone().from.clone().unwrap();
            let mut map = self.peer_map.lock().await;
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
        tokio::spawn(async move {
            done_rx.recv().await;
            log::info!("Received done signal!");
            peer_connection.close().await.unwrap();
        });
        Ok(())
    }

    async fn new(my_name: String, url: String) -> Result<Self> {
        // Connect to the signaling server
        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();
        let write = Arc::new(Mutex::new(write));

        let register_msg = Message {
            r#type: "register".to_string(),
            name: Some(my_name.clone()),
            peer_type: Some("server".to_string()),
            session: None,
            target: None,
            data: None,
            from: None,
        };
        write
            .lock()
            .await
            .send(tungstenite::Message::Text(serde_json::to_string(
                &register_msg,
            )?))
            .await?;

        // Everything below is the WebRTC-rs API! Thanks for using it ❤️.

        // Create a MediaEngine object to configure the supported codec
        let mut m = MediaEngine::default();

        // Register default codecs
        m.register_default_codecs()?;

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
        // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
        // for each PeerConnection.
        let mut registry = Registry::new();

        // Use the default set of Interceptors
        registry = register_default_interceptors(registry, &mut m)?;

        // Create the API object with the MediaEngine
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // Prepare the configuration
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let session_map: SessionMap = Arc::new(std::sync::Mutex::new(HashMap::default()));
        let peer_map: PeerMap = Arc::new(Mutex::new(HashMap::default()));

        Ok(Self {
            api,
            config,
            session_map,
            my_name,
            write,
            read,
            peer_map,
        })
    }

    pub async fn handle_ws_message(&self, text: String) -> Result<()> {
        let message: Message = serde_json::from_str(&text)?;
        match message.r#type.as_str() {
            "connection_request" => {
                let user_name = message.from.ok_or(anyhow!("No username provided"))?;
                log::info!("Connection request from {}", user_name);
            }
            "signal" => {
                self.handle_signal(message).await?;
            }
            "candidate" => {
                let user_name = message.from.ok_or(anyhow!("No username provided"))?;

                let map = self.peer_map.lock().await;
                let peer = map.get(&user_name).ok_or(anyhow!("Peer not found"))?;
                let data = message.data.ok_or(anyhow!("No data provided"))?;
                let candidate: RTCIceCandidateInit =
                    serde_json::from_str::<RTCIceCandidateInit>(&data)?;
                peer.peer_connection.add_ice_candidate(candidate).await?;
            }
            _ => {}
        }
        Ok(())
    }
}

pub async fn connect(my_name: String, url: String) -> Result<()> {
    let mut state = State::new(my_name, url).await?;
    // Handle incoming messages
    // let write_clone = write.clone();
    while let Some(msg) = state.read.next().await {
        match msg {
            Result::Ok(tungstenite::Message::Text(text)) => {
                if let Err(e) = state.handle_ws_message(text).await {
                    log::error!("Error while handling websocket message {}", e.to_string());
                }
            }
            Result::Err(e) => {
                log::error!("Error with websocket message {}", e.to_string());
            }
            _ => {
                log::warn!("Websocket message type is not supported ");
            }
        }
    }

    Result::Ok(())
}
