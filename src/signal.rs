use crate::peer::{Peer, PeerMap};
use crate::recording::add_video;
use crate::shell::SessionMap;
use crate::state::State;
use anyhow::{anyhow, Ok, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use virtual_display::VirtualDisplayManager;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::signaling_state::RTCSignalingState;
use webrtc::peer_connection::{self, RTCPeerConnection};

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub r#type: String,
    pub name: Option<String>,
    pub target: Option<String>,
    pub data: Option<String>,
    pub peer_type: Option<String>,
    pub from: Option<String>,
}

pub trait Signaling {
    fn send(&self, msg: String);
    fn next(&self) -> impl Future<Output = Option<String>>;
}

pub struct WsSignaling {
    sender: mpsc::Sender<String>,
    receiver: Arc<Mutex<mpsc::Receiver<Option<String>>>>,
}

impl WsSignaling {
    pub async fn new(url: &str) -> Result<Self> {
        let (connect_tx, mut connect_rx) = mpsc::channel(32);
        let (msg_tx, msg_rx) = mpsc::channel(32);

        let (socket, _) = connect_async(url).await?;
        let (write, read) = socket.split();

        // Sender task
        tokio::spawn(async move {
            let mut write = write;
            while let Some(msg) = connect_rx.recv().await {
                write.send(tungstenite::Message::Text(msg)).await.unwrap();
            }
        });

        // Receiver task
        tokio::spawn(async move {
            let mut read = read;
            let msg_tx = msg_tx.clone();

            while let Some(msg) = read.next().await {
                match msg {
                    Result::Ok(tungstenite::Message::Text(text)) => {
                        if let Err(e) = msg_tx.send(Some(text)).await {
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
                // let text_msg = match msg {
                //     Result::Ok(tungstenite::Message::Text(text)) => Some(text),
                //     _ => None,
                // };
            }
            let _ = msg_tx.send(None).await;
        });

        Ok(WsSignaling {
            sender: connect_tx,
            receiver: Arc::new(Mutex::new(msg_rx)),
        })
    }
}

impl Signaling for WsSignaling {
    fn send(&self, msg: String) {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            sender.send(msg).await.unwrap();
        });
    }

    async fn next(&self) -> Option<String> {
        // self.receiver.clone();
        // async move {
        self.receiver.lock().await.recv().await.unwrap_or(None)
        // }
    }
}

impl State<WsSignaling> {
    // async fn create_connection(&self) -> Result<()> {

    // }
    pub async fn create_peer_connection<'a>(
        &self,
        target: String,
    ) -> Result<(Arc<RTCPeerConnection>, tokio::sync::mpsc::Receiver<()>)> {
        let peer_connection = Arc::new(self.api.new_peer_connection(self.config.clone()).await?);

        let (done_tx, done_rx) = tokio::sync::mpsc::channel::<()>(1);

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

        let my_name = self.my_name.clone();
        // let write_clone = self.write.clone();
        let signaling = self.signaling.clone();

        let t = target.clone();
        peer_connection.on_ice_candidate(Box::new(move |candidate| {
            // let write_clone = write_clone.clone();
            let signaling = signaling.clone();
            if let Some(candidate) = candidate {
                let signal_msg = Message {
                    r#type: "candidate".to_string(),
                    name: Some(my_name.to_string()),
                    target: Some(t.clone()),
                    data: Some(serde_json::to_string(&candidate.to_json().unwrap()).unwrap()),
                    peer_type: None,
                    from: None,
                };
                tokio::spawn(async move {
                    // let mut write_guard = .lock().await;
                    signaling.send(serde_json::to_string(&signal_msg).unwrap());
                });
            }

            Box::pin(async {})
        }));

        let s = self.signaling.clone();

        let p = peer_connection.clone();
        let m = self.my_name.clone();
        let t = target.clone();

        peer_connection.on_negotiation_needed(Box::new(move || {
            let s = s.clone();
            let p = p.clone();
            let m = m.clone();
            let t = t.clone();

            Box::pin(async move {
                // if p.signaling_state() != RTCSignalingState::Stable {
                //     // We already have a pending offer/answer exchange.
                //     // Option A: Do nothing and wait until it’s stable again.
                //     // Option B: Attempt a rollback (advanced usage).
                //     return;
                // }
                dbg!(p.signaling_state());
                let local_desc = p.create_offer(None).await.unwrap();
                // if p.signaling_state() != RTCSignalingState::Stable {
                //     // We already have a pending offer/answer exchange.
                //     // Option A: Do nothing and wait until it’s stable again.
                //     // Option B: Attempt a rollback (advanced usage).
                //     return;
                // }
                p.set_local_description(local_desc.clone()).await.unwrap();

                let signal_msg = Message {
                    r#type: "offer".to_string(),
                    name: Some(m.clone().to_string()),
                    target: Some(t.clone()),
                    data: Some(serde_json::to_string(&local_desc).unwrap()),
                    peer_type: None,
                    from: None,
                };

                s.send(serde_json::to_string(&signal_msg).unwrap());
            })
        }));

        Ok((peer_connection, done_rx))
    }

    async fn handle_answer(&self, message: Message) -> Result<()> {
        let map = self.peer_map.lock().await;
        let Some(peer) = map.get(&message.from.clone().ok_or(anyhow!("no from"))?) else {
            return Err(anyhow!("peer not found"));
        };

        let data = message.clone().data.ok_or(anyhow!("No data in message"))?;
        let sdp: RTCSessionDescription = serde_json::from_str::<RTCSessionDescription>(&data)?;

        peer.peer_connection.set_remote_description(sdp).await?;

        Ok(())
    }

    async fn init_peer_connection(self: Arc<Self>, message: Message) -> Result<()> {
        {
            let user_name = message.clone().from.clone().unwrap();
            let mut map = self.peer_map.lock().await;
            if let Some(_) = map.get(&user_name) {
                log::error!("Peer exists");
                // Session exists
            } else {
                // Create a new RTCPeerConnection
                let session_map = self.session_map.clone();

                let message_clone = message.clone();
                let from = message_clone.clone().from;
                let (peer_connection, mut done_rx) = self
                    .create_peer_connection(from.ok_or(anyhow!("no from"))?)
                    .await?;

                // Register data channel creation handling
                let self_ref = self.clone();
                let pc = peer_connection.clone();
                peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
                    let pc = pc.clone();
                    let self_ref = self_ref.clone();
                    if let Err(e) = self_ref.on_data_channel(
                        pc,
                        d,
                        session_map.clone(),
                    ) {
                        log::error!("Failed to handle data channel: {}", e.to_string())
                    }
                    Box::pin(async {})
                }));
                map.insert(
                    user_name,
                    Peer {
                        peer_connection: peer_connection.clone(),
                    },
                );
            }
        }

        Ok(())
    }

    async fn handle_offer(self: Arc<Self>, message: Message) -> Result<()> {
        self.clone().init_peer_connection(message.clone()).await?;
        let map = self.peer_map.lock().await;
        let Some(peer) = map.get(&message.from.clone().ok_or(anyhow!("no from"))?) else {
            return Err(anyhow!("peer not found"));
        };
        let peer_connection = peer.peer_connection.clone();

        let data = message.clone().data.ok_or(anyhow!("No data in message"))?;
        let sdp: RTCSessionDescription = serde_json::from_str::<RTCSessionDescription>(&data)?;

        peer_connection.set_remote_description(sdp).await?;

        let my_name = self.my_name.clone();

        if peer_connection
            .remote_description()
            .await
            .ok_or(anyhow!("No remote description"))?
            .sdp_type
            == RTCSdpType::Offer
        {
            let local_desc = peer_connection.create_answer(None).await?;
            peer_connection.set_local_description(local_desc).await?;
            let local_desc = peer_connection
                .local_description()
                .await
                .ok_or(anyhow!("Can't get local description"))?;

            let signal_msg = Message {
                r#type: "answer".to_string(),
                name: Some(my_name.clone().to_string()),
                target: Some(message.clone().from.ok_or(anyhow!("No from provides"))?),
                data: Some(serde_json::to_string(&local_desc)?),
                peer_type: None,
                from: None,
            };
            self.signaling.send(serde_json::to_string(&signal_msg)?);
        }

        Ok(())
    }

    pub async fn new(my_name: String, peer_type: String, url: String) -> Result<Self> {
        let signaling = Arc::new(WsSignaling::new(url.as_str()).await?);

        // Connect to the signaling server
        // let (ws_stream, _) = connect_async(url).await?;
        // let (write, read) = ws_stream.split();
        // let write = Arc::new(Mutex::new(write));

        let register_msg = Message {
            r#type: "register".to_string(),
            name: Some(my_name.clone()),
            peer_type: Some(peer_type.to_string()),
            target: None,
            data: None,
            from: None,
        };
        signaling.send(serde_json::to_string(&register_msg)?);

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

        let display_manager = Arc::new(VirtualDisplayManager::new().await?);
        Ok(Self {
            api,
            config,
            session_map,
            my_name,
            signaling,
            peer_map,
            display_manager,
        })
    }

    pub async fn handle_ws_message(self: Arc<Self>, text: String) -> Result<()> {
        let message: Message = serde_json::from_str(&text)?;
        match message.r#type.as_str() {
            "connection_request" => {
                let user_name = message.from.ok_or(anyhow!("No username provided"))?;
                log::info!("Connection request from {}", user_name);
            }
            "offer" => {
                self.handle_offer(message).await?;
            }
            "answer" => {
                self.handle_answer(message).await?;
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

    pub async fn signal_loop(self: Arc<Self>) {
        let signaling = self.clone().signaling.clone();

        while let Some(msg) = signaling.next().await {
            let state = self.clone();
            if let Err(e) = state.handle_ws_message(msg).await {
                log::error!("Error while handling ws message: {}", e.to_string())
            }
        }
    }
}

pub async fn connect(my_name: String, peer_type: String, url: String) -> Result<()> {
    let state = Arc::new(State::new(my_name, peer_type, url).await?);
    state.signal_loop().await;

    Result::Ok(())
}
