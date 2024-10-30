use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use clap::{AppSettings, Arg, Command};
use shell::handle_pty;
use tokio::sync::mpsc::{self};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

pub mod shell;
pub mod signal;

fn on_data_channel(d: Arc<RTCDataChannel>) {
    let d_label = d.label().to_owned();
    let d_id = d.id();
    println!("New DataChannel {d_label} {d_id}");

    let (tx, mut rx) = mpsc::channel::<String>(100);
    let (ptx, prx) = mpsc::channel::<String>(100);

    // Start the PTY handler in its own thread
    tokio::spawn(async {
        handle_pty(tx, prx).await;
    });

    // Register channel opening handling
    let d2 = Arc::clone(&d);
    // let d_label2 = d_label.clone();
    // let d_id2 = d_id;

    d.on_close(Box::new(move || {
        println!("Data channel closed");
        Box::pin(async {})
    }));
    d.on_open(Box::new(move || {
        let d_clone = Arc::clone(&d2); // Clone the Arc to use in the async block
        Box::pin(async move {
            // Launch a task to handle sending messages received via the channel
            tokio::spawn(async move {
                while let Some(message) = rx.recv().await {
                    if d_clone.send_text(message).await.is_err() {
                        eprintln!("Failed to send message over data channel");
                        break;
                    }
                }
            });

            // Example of sending a message directly, perhaps as an initial message
            if d2
                .send_text("Hello from data channel!".to_string())
                .await
                .is_err()
            {
                eprintln!("Failed to send initial message.");
            }
        })
    }));

    // Register text message handling
    d.on_message(Box::new(move |msg: DataChannelMessage| {
        let ptx_clone = ptx.clone(); // Clone the sender for use in the async context
        let msg_str = String::from_utf8(msg.data.to_vec()).unwrap(); // Convert the received message to a String

        Box::pin(async move {
            // Send the message to the PTY task asynchronously
            if let Err(e) = ptx_clone.send(msg_str).await {
                eprintln!("Failed to send message to PTY: {}", e);
            }
        })
    }));
}

fn app_setup() {
    let mut app = Command::new("data-channels")
        .version("0.1.0")
        .author("Wgmlgz")
        .about("Shell web-rtc server")
        .setting(AppSettings::DeriveDisplayOrder)
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("FULLHELP")
                .help("Prints more detailed help information")
                .long("fullhelp"),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Prints debug log information"),
        );

    let matches = app.clone().get_matches();

    if matches.is_present("FULLHELP") {
        app.print_long_help().unwrap();
        std::process::exit(0);
    }

    let debug = matches.is_present("debug");
    if debug {
        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} [{}] {} - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    chrono::Local::now().format("%H:%M:%S.%6f"),
                    record.args()
                )
            })
            .filter(None, log::LevelFilter::Trace)
            .init();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    app_setup();
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

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            println!("Peer Connection has gone to failed exiting");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }));

    // Register data channel creation handling
    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        on_data_channel(d);
        Box::pin(async {})
    }));

    // Wait for the offer to be pasted
    let line = signal::must_read_stdin()?;
    let desc_data = signal::decode(line.as_str())?;
    let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // Set the remote SessionDescription
    peer_connection.set_remote_description(offer).await?;

    // Create an answer
    let answer = peer_connection.create_answer(None).await?;

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(answer).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    // Output the answer in base64 so we can paste it in browser
    if let Some(local_desc) = peer_connection.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = signal::encode(&json_str);
        println!("{b64}");
    } else {
        println!("generate local_description failed!");
    }

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    peer_connection.close().await?;

    Ok(())
}
