use std::io::Write;
use std::sync::Arc;

use anyhow::{Ok, Result};
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

pub mod shell;
pub mod signal;



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


    signal::connect().await?;
    // // Wait for the offer to be pasted
    // let line = signal::must_read_stdin()?;
    // let desc_data = signal::decode(line.as_str())?;
    // let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // // Set the remote SessionDescription
    // peer_connection.set_remote_description(offer).await?;

    // // Create an answer
    // let answer = peer_connection.create_answer(None).await?;

    // // Create channel that is blocked until ICE Gathering is complete
    // let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // // Sets the LocalDescription, and starts our UDP listeners
    // peer_connection.set_local_description(answer).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    // let _ = gather_complete.recv().await;

    // Output the answer in base64 so we can paste it in browser
    // if let Some(local_desc) = peer_connection.local_description().await {
    //     let json_str = serde_json::to_string(&local_desc)?;
    //     let b64 = signal::encode(&json_str);
    //     println!("{b64}");
    // } else {
    //     println!("generate local_description failed!");
    // }



    Ok(())
}
