mod control;
mod receive;
mod receiver;
mod transmit;

use crate::control::control;
use crate::receive::receive;
use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser, Debug, Clone)]
#[clap(version, long_about = None, propagate_version = true)]
#[clap(about = "Transmit/Receive IPFS block stream")]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    #[clap(about = "Receive files")]
    Receive {
        /// The address to listen for the file data on
        listen_address: String,
    },
    #[clap(about = "Control mode")]
    Control { listen_address: String },
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        match &self.command {
            Commands::Receive { listen_address } => receive(listen_address).await?,
            Commands::Control { listen_address } => control(listen_address).await?,
        }
        Ok(())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();
    cli.run().await
}
