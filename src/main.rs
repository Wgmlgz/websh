use anyhow::{Ok, Result};

use clap::Parser;

pub mod shell;
pub mod signal;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    name: Option<String>,
    url: Option<String>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    signal::connect(
        cli.name.unwrap_or("server1".into()),
        cli.url.unwrap_or("ws://localhost:8002".into()),
    )
    .await?;

    Ok(())
}
