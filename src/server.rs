use anyhow::Result;

use clap::Parser;
use env_logger::Env;

pub mod peer;
pub mod port;
pub mod shell;
pub mod signal;
pub mod convert;
pub mod recording;

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
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    loop {
        log::info!("Starting app");
        if let Err(e) = signal::connect(
            cli.name.clone().unwrap_or("server1".into()),
            "server".into(),
            cli.url
                .clone()
                .unwrap_or("wss://websh.amogos.pro/signaling".into()),
        )
        .await
        {
            log::error!("Error while running app: {}", e.to_string());
        };
    }
}
