use crate::shell::{handle_pty, PTYSession, SessionMap};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::connect_async;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
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

fn on_data_channel(d: Arc<RTCDataChannel>, session: Option<String>, session_map: SessionMap) {
    let session_id = session.unwrap_or_else(|| "default".to_string());

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    let d_label = d.label().to_owned();
    let d_id = d.id();
    println!("New DataChannel {d_label} {d_id}");

    // Check if session already exists
    let pty_session = {
        let mut map = session_map.lock().unwrap();
        if let Some(pty_session) = map.get(&session_id) {
            // Session exists
            pty_session.clone()
        } else {
            // Session does not exist, create new PTYSession
            let (to_pty_tx, to_pty_rx) = mpsc::channel::<String>(100);
            let (from_pty_tx, _) = broadcast::channel::<String>(100);
            let (done_tx, done_rx) = mpsc::channel::<()>(1);

            // Start PTY handler
            tokio::spawn({
                let from_pty_tx = from_pty_tx.clone();
                async move {
                    handle_pty(from_pty_tx, to_pty_rx, done_rx).await;
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
        println!("Data channel closed");
        let _ = done_tx.try_send(());
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
                    if d_clone.send_text(message).await.is_err() {
                        eprintln!("Failed to send message over data channel");
                        break;
                    }
                }
            });
        })
    }));

    // Register text message handling
    d.on_message(Box::new(move |msg: DataChannelMessage| {
        let ptx_clone = to_pty.clone(); // Clone the sender for use in the async context
        let msg_str = String::from_utf8(msg.data.to_vec()).unwrap(); // Convert the received message to a String

        Box::pin(async move {
            // Send the message to the PTY task asynchronously
            if let Err(e) = ptx_clone.send(msg_str).await {
                eprintln!("Failed to send message to PTY: {}", e);
            }
        })
    }));
}

#[derive(Clone)]
pub struct Peer {
    pub peer_connection: Arc<RTCPeerConnection>, // To signal done
}

pub type PeerMap = Arc<Mutex<HashMap<String, Peer>>>;

pub async fn connect(my_name: String, url: String) -> Result<()> {
    // Connect to the signaling server
    let (ws_stream, _) = connect_async(url).await?;
    let (write, mut read) = ws_stream.split();
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
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&register_msg)?,
        ))
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
    // Handle incoming messages
    // let write_clone = write.clone();
    while let Some(msg) = read.next().await {
        match msg {
            Result::Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                dbg!(&text);
                let session_map = session_map.clone();

                let message: Message = serde_json::from_str(&text)?;
                match message.r#type.as_str() {
                    "connection_request" => {
                        let user_name = message.from.unwrap();
                        println!("Connection request from {}", user_name);
                        // Save the user's name to send signaling messages
                        // Implement logic to accept/reject the connection if needed
                    }
                    "signal" => {
                        // Handle signaling messages from the user
                        let data = message.clone().data.unwrap();
                        let sdp: RTCSessionDescription =
                            serde_json::from_str::<RTCSessionDescription>(&data)?;

                        // Create a new RTCPeerConnection
                        let peer_connection =
                            Arc::new(api.new_peer_connection(config.clone()).await?);

                        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

                        // Set the handler for Peer connection state
                        // This will notify you when the peer has connected/disconnected
                        peer_connection.on_peer_connection_state_change(Box::new(
                            move |s: RTCPeerConnectionState| {
                                println!("Peer Connection State has changed: {s}");

                                if s == RTCPeerConnectionState::Failed {
                                    // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
                                    // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
                                    // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
                                    println!("Peer Connection has gone to failed exiting");
                                    let _ = done_tx.try_send(());
                                }

                                Box::pin(async {})
                            },
                        ));

                        let message_clone = message.clone();
                        // Register data channel creation handling
                        peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
                            on_data_channel(d, message_clone.clone().session, session_map.clone());
                            Box::pin(async {})
                        }));

                        peer_connection.set_remote_description(sdp).await?;

                        let my_name = my_name.clone();

                        if peer_connection.remote_description().await.unwrap().sdp_type
                            == RTCSdpType::Offer
                        {
                            let answer = peer_connection.create_answer(None).await?;
                            peer_connection.set_local_description(answer).await?;
                            let local_desc = peer_connection.local_description().await.unwrap();

                            let signal_msg = Message {
                                r#type: "signal".to_string(),
                                name: Some(my_name.clone().to_string()),
                                target: Some(message.clone().from.unwrap()),
                                data: Some(serde_json::to_string(&local_desc)?),
                                session: None,
                                peer_type: None,
                                from: None,
                            };
                            write
                                .lock()
                                .await
                                .send(tokio_tungstenite::tungstenite::Message::Text(
                                    serde_json::to_string(&signal_msg)?,
                                ))
                                .await?;
                        }
                        let write = write.clone();

                        let message_clone = message.clone();
                        peer_connection.on_ice_candidate(Box::new(move |candidate| {
                            let from = message_clone.clone().from;
                            let my_name = my_name.clone();
                            if let Some(candidate) = candidate {
                                let write_clone = write.clone();
                                tokio::spawn(async move {
                                    let signal_msg = Message {
                                        r#type: "candidate".to_string(),
                                        name: Some(my_name.to_string()),
                                        target: Some(from.clone().unwrap()),
                                        data: Some(serde_json::to_string(&candidate.to_json().unwrap()).unwrap()),
                                        session: None,
                                        peer_type: None,
                                        from: None,
                                    };

                                    let mut write_guard = write_clone.lock().await;

                                    write_guard
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
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
                            let mut map = peer_map.lock().await;
                            if let Some(_) = map.get(&user_name) {
                                eprintln!("Peer exists");
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
                            println!("PenDING");
                            done_rx.recv().await;
                            println!("received done signal!");

                            println!("connection done");
                            peer_connection.close().await.unwrap();
                        });
                        ()
                    }
                    "candidate" => {
                        let user_name = message.from.unwrap();

                        let map = peer_map.lock().await;
                        if let Some(peer) = map.get(&user_name) {
                            let data = message.data.unwrap();
                            let candidate: RTCIceCandidateInit =
                                serde_json::from_str::<RTCIceCandidateInit>(&data)?;
                            if let Err(e) = peer.peer_connection.add_ice_candidate(candidate).await
                            {
                                println!("error adding ice candiate! {:}", e.to_string());
                            }
                        } else {
                            eprintln!("peer not found")
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Result::Ok(())
}
