mod receive;
mod transmit;

use crate::receive::receive;
use crate::transmit::transmit;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
    #[clap(about = "Transmit a file")]
    Transmit {
        /// The path to a file to be transmitted
        path: PathBuf,
        /// The address to transmit the file to
        target_address: String,
    },
    #[clap(about = "Receive a file")]
    Receive {
        /// The path to a file where received blocks will be output
        path: PathBuf,
        /// The address to listen for the file on
        listen_address: String,
    },
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        match &self.command {
            Commands::Transmit {
                path,
                target_address,
            } => transmit(path, target_address).await?,
            Commands::Receive {
                path,
                listen_address,
            } => receive(path, listen_address).await?,
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
