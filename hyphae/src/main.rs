mod config;

use anyhow::Result;
use clap::Parser;
use config::Config;
use tracing::{info, Level};
#[derive(Parser, Debug)]
#[clap(about = "Hyphae, a filament between Mycelie and Kubo")]
struct Args {
    /// Path to config file
    config_path: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();

    let cfg: Config = Config::parse(args.config_path).expect("Configuration parsing failed");

    info!("Hyphae starting");
    info!("Connecting to myceli@{}", cfg.myceli_address);
    info!("Connecting to kubo@{}", cfg.kubo_address);

    Ok(())
}
