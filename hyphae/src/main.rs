mod config;
mod kubo_api;
mod myceli_api;

use std::ops::Sub;

use anyhow::Result;
use clap::Parser;
use config::Config;
use kubo_api::KuboApi;
use myceli_api::MyceliApi;
use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;
use tracing::{error, info, warn, Level};

pub const RAW_CODEC_PREFIX: &str = "bafkrei";
pub const DAG_PB_CODEC_PREFIX: &str = "bafybei";

#[derive(Parser, Debug)]
#[clap(about = "Hyphae, a filament between Myceli and Kubo")]
struct Args {
    /// Path to config file
    config_path: Option<String>,
}

fn get_missing_blocks(
    mut myceli_blocks: Vec<String>,
    kubo_blocks: HashSet<String>,
) -> HashSet<String> {
    // Kubo will take blocks with the dag-pb codec and convert them to raw blocks, which throws off the diff.
    // So we will replace any cids with the dag-pb codec with a raw codec for the purposes of the diff,
    // but the actual sync will be done on the dag-pb cids.
    let mut raw_from_dag_pb_blocks: Vec<String> = vec![];
    for block in myceli_blocks.iter_mut() {
        if block.starts_with(DAG_PB_CODEC_PREFIX) {
            let pb_to_raw_cid = block.replace(DAG_PB_CODEC_PREFIX, RAW_CODEC_PREFIX);
            raw_from_dag_pb_blocks.push(pb_to_raw_cid.to_string());
            *block = pb_to_raw_cid.to_string();
        }
    }

    let myceli_blocks = HashSet::from_iter(myceli_blocks.into_iter());
    let mut missing_blocks = myceli_blocks.sub(&kubo_blocks);
    for cid in raw_from_dag_pb_blocks {
        if missing_blocks.contains(&cid) {
            missing_blocks.remove(&cid);
            missing_blocks.insert(cid.replace(RAW_CODEC_PREFIX, DAG_PB_CODEC_PREFIX));
        }
    }
    missing_blocks
}

fn sync_blocks(kubo: &KuboApi, myceli: &MyceliApi) -> Result<()> {
    info!("Begin syncing myceli blocks to kubo");
    let myceli_blocks = myceli.get_available_blocks()?;
    let kubo_blocks = kubo.get_local_blocks()?;
    let missing_blocks = get_missing_blocks(myceli_blocks, kubo_blocks);
    let mut all_blocks_synced = true;

    for cid in missing_blocks {
        info!("Syncing block {cid} from myceli to kubo");
        let block = match myceli.get_block(&cid) {
            Ok(block) => block,
            Err(e) => {
                // TODO: Surface more error info here. Connection error vs internal myceli error?
                error!("Error retrieving block {cid} from myceli: {e}");
                all_blocks_synced = false;
                continue;
            }
        };
        if let Err(e) = kubo.put_block(&cid, &block) {
            error!("Error sending block {cid} to kubo: {e}");
            all_blocks_synced = false;
        }
    }

    if all_blocks_synced {
        info!("All myceli blocks are synced");
    } else {
        warn!("Not all myceli blocks were able to sync, check logs for specific errors");
    }

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
    let myceli = MyceliApi::new(
        &cfg.myceli_address,
        &cfg.listen_to_myceli_address,
        cfg.myceli_mtu,
        cfg.chunk_transmit_throttle,
    )
    .expect("Failed to create MyceliAPi");

    loop {
        if kubo.check_alive() && myceli.check_alive() {
            if let Err(e) = sync_blocks(&kubo, &myceli) {
                error!("Error during blocks sync: {e}");
            }
        }
        sleep(Duration::from_millis(cfg.sync_interval));
    }
}
