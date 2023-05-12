use std::path::PathBuf;

use crate::pack::pack;
use crate::unpack::unpack;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[clap(version, long_about = None, propagate_version = true)]
#[clap(about = "CAR packer/unpacker based on Iroh")]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    #[clap(about = "Pack a file into a CAR")]
    Pack {
        /// The path to a file to be CAR packed
        path: PathBuf,
        // The path to the CAR output file
        output: PathBuf,
    },
    #[clap(about = "Unpack a CAR into a file")]
    Unpack {
        /// The path to a CAR file to be unpacked
        path: PathBuf,
        /// The path to the unpacked output file
        output: PathBuf,
    },
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        self.cli_command().await?;

        Ok(())
    }

    async fn cli_command(&self) -> Result<()> {
        match &self.command {
            Commands::Pack { path, output } => {
                if !path.is_file() {
                    anyhow::bail!("{} is not a file", path.display());
                }
                println!("Packing {} into {}", path.display(), output.display());
                pack(path, output).await?;
            }
            Commands::Unpack { path, output } => {
                if !path.is_file() {
                    anyhow::bail!("{} is not a file", path.display());
                }
                println!("Unpacking {} into {}", path.display(), output.display());
                unpack(path, output).await?;
            }
        };

        Ok(())
    }
}
