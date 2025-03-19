use anyhow::Result;

use clap::Parser;
use env_logger::Env;
use gstreamer as gst;

pub mod convert;
pub mod peer;
pub mod port;
pub mod recording;
pub mod shell;
pub mod signal;
pub mod state;
pub mod control;
pub mod utils;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    name: Option<String>,

    #[arg(short, long)]
    url: Option<String>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    log::info!("PLSSSSS");


    // essentially just mix the goblin syrup with bathsalts
    std::env::set_var("GST_DEBUG", "3");
    std::env::set_var(
        "GST_PLUGIN_PATH",
        "C:\\Program Files\\gstreamer\\1.0\\msvc_x86_64\\lib\\gstreamer-1.0",
    );
    gst::init()?;
    log::info!("PLSSSSS222");
    // Ok(())
    // loop {}
    log::info!("PLSSSSS3333");
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    loop {
        log::info!("Starting app");
        if let Err(e) = signal::connect(
            cli.name.clone().unwrap_or("server1".into()),
            "server".into(),
            cli.url
                .clone()
                .unwrap_or("ws://amogos.pro:8002/signaling".into()),
        )
        .await
        {
            log::error!("Error while running app: {}", e.to_string());
        };
    }
}
