mod config;
mod indexer;
mod kubo_api;
mod myceli_api;

use anyhow::Result;
use clap::Parser;
use config::Config;
use indexer::Indexer;
use kubo_api::KuboApi;
use log::{debug, error, info, warn};
use messages::TransmissionBlock;
use myceli_api::MyceliApi;
use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

pub const RAW_CODEC_PREFIX: &str = "bafkrei";
pub const DAG_PB_CODEC_PREFIX: &str = "bafybei";

#[derive(Parser, Debug)]
#[clap(about = "Hyphae, a filament between Myceli and Kubo")]
struct Args {
    /// Path to config file
    config_path: Option<String>,
}

type Synchronized = BTreeMap<String, TransmissionBlock>;

fn get_missing_blocks(myceli_blocks: Vec<String>, kubo_blocks: &Synchronized) -> Vec<String> {
    myceli_blocks
        .into_iter()
        .filter_map(|b| {
            if kubo_blocks.contains_key(&b) {
                None
            } else {
                Some(b)
            }
        })
        .collect()
}

fn sync_blocks(kubo: &KuboApi, myceli: &MyceliApi, kubo_blocks: &mut Synchronized) -> Result<bool> {
    let myceli_blocks = myceli.get_available_blocks()?;
    let missing_blocks = get_missing_blocks(myceli_blocks, kubo_blocks);
    if missing_blocks.is_empty() {
        return Ok(false);
    }
    debug!(
        "Begin syncing {} myceli blocks to kubo",
        missing_blocks.len()
    );
    let mut all_blocks_synced = true;

    for cid in missing_blocks {
        debug!("Looking to synchronize block {cid} from myceli to kubo");
        let block = match myceli.get_block(&cid) {
            Ok(block) => block,
            Err(e) => {
                // TODO: Surface more error info here. Connection error vs internal myceli error?
                error!("Error retrieving block {cid} from myceli: {e}");
                all_blocks_synced = false;
                continue;
            }
        };
        match kubo.put_block(&cid, &block, true) {
            Err(e) => {
                error!("Error sending block {cid} to kubo: {e}");
                all_blocks_synced = false;
            }
            Ok(resp) => {
                debug!("Synchronized {} and got {:?}", &cid, resp);
                kubo_blocks.insert(cid, block.clone());
            }
        }
    }

    if all_blocks_synced {
        info!("All myceli blocks are synced.");
        Ok(true)
    } else {
        warn!("Not all myceli blocks were able to sync, check logs for specific errors");
        Ok(false)
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    let cfg: Config = Config::parse(args.config_path).expect("Configuration parsing failed");

    info!("Hyphae starting");
    info!("Connecting to myceli@{}", cfg.myceli_address);
    info!("Connecting to kubo@{}", cfg.kubo_address);

    let kubo = KuboApi::new(&cfg.kubo_address);
    let mut myceli_api: Option<MyceliApi> = None;
    let mut indexer = Indexer::new(&kubo);
    let mut synced = Synchronized::default();
    let mut miss = 0;
    loop {
        if let Some(myceli) = &myceli_api {
            if kubo.check_alive() && myceli.check_alive() {
                match sync_blocks(&kubo, myceli, &mut synced) {
                    Err(e) => error!("Error during blocks sync: {e}"),
                    Ok(true) => debug!("Synchronization happened."),
                    Ok(false) => {
                        if let Err(e) = indexer.step(&synced) {
                            error!("Trouble indexing: {:?}", &e);
                        } else {
                            miss = 0;
                        }
                    }
                }
            } else if miss > 9 {
                info!("Connection to Myceli failed {} times, resetting.", miss);
                myceli_api = None;
                miss = 0;
            } else {
                miss += 1;
            }
        } else {
            info!("Creating a Myceli API object...");
            myceli_api = MyceliApi::new(
                &cfg.myceli_address,
                &cfg.listen_to_myceli_address,
                cfg.myceli_mtu,
                cfg.chunk_transmit_throttle,
            )
            .ok();
        }
        sleep(Duration::from_millis(cfg.sync_interval));
    }
}
