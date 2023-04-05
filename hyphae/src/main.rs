mod config;
mod kubo_api;
mod myceli_api;

use std::ops::Sub;

use anyhow::{anyhow, Result};
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

fn sync_blocks(kubo: &KuboApi, myceli: &MyceliApi) -> Result<()> {
    info!("Begin syncing myceli blocks to kubo");
    let myceli_blocks = myceli.get_available_blocks()?;
    let kubo_blocks = kubo.get_local_blocks()?;

    println!("myceli {:#?}", myceli_blocks);
    println!("kubo {:#?}", kubo_blocks);

    let missing_blocks = myceli_blocks.sub(&kubo_blocks);
    println!("miss {:#?}", missing_blocks);
    for cid in missing_blocks {
        info!("Syncing block {cid} from myceli to kubo");
        let block = myceli
            .get_block(&cid)
            .map_err(|e| anyhow!("Error getting block {e}"))?;
        kubo.put_block(&cid, &block)?;
    }

    info!("All myceli blocks are synced");

    Ok(())
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

    sync_blocks(&kubo, &myceli)?;

    Ok(())
}
