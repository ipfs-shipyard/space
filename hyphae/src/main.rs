mod config;
mod kubo_api;
mod myceli_api;

use anyhow::Result;
use clap::Parser;
use config::Config;
use kubo_api::KuboApi;
use myceli_api::MyceliApi;
use tracing::{info, warn, Level};

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

    let kubo = KuboApi::new(&cfg.kubo_address);
    let myceli = MyceliApi::new(&cfg.myceli_address);

    if kubo.check_alive().is_err() {
        warn!("Could not contact Kubo at this time");
    }

    if myceli.check_alive().is_err() {
        warn!("Could not contact Myceli at this time");
    }

    Ok(())
}
