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

const raw_cid: &str = "bafkrei";
const dag_pb_cid: &str = "bafybei";

#[derive(Parser, Debug)]
#[clap(about = "Hyphae, a filament between Mycelie and Kubo")]
struct Args {
    /// Path to config file
    config_path: Option<String>,
}

fn sync_blocks(kubo: &KuboApi, myceli: &MyceliApi) -> Result<()> {
    info!("Begin syncing myceli blocks to kubo");
    let mut myceli_blocks = myceli.get_available_blocks()?;
    let kubo_blocks = kubo.get_local_blocks()?;

    // Kubo will take blocks with the dag-pb codec
    // and convert them to raw blocks, which throws off the sync.
    // So we will replace any cids with the dag-pb codec with a raw codec
    // for the purposes of the sync comparison, but the actual sync will be done
    // on the dag-pb cids.
    let mut raw_from_dag_pb_blocks: Vec<String> = vec![];
    for block in myceli_blocks.iter_mut() {
        if block.starts_with(dag_pb_cid) {
            let pb_to_raw_cid = block.replace(dag_pb_cid, raw_cid);
            raw_from_dag_pb_blocks.push(pb_to_raw_cid.to_string());
            *block = pb_to_raw_cid.to_string();
        }
    }

    let myceli_blocks = HashSet::from_iter(myceli_blocks.into_iter());

    println!("myceli {:#?}", myceli_blocks);
    println!("kubo {:#?}", kubo_blocks);

    let missing_blocks = myceli_blocks.sub(&kubo_blocks);
    println!("miss {:#?}", missing_blocks);
    for cid in missing_blocks {
        let cid = if raw_from_dag_pb_blocks.contains(&cid) {
            cid.replace(raw_cid, dag_pb_cid)
        } else {
            cid
        };
        info!("Syncing block {cid} from myceli to kubo");
        let block = match myceli.get_block(&cid) {
            Ok(block) => block,
            Err(e) => {
                error!("Error retrieving block {cid} from myceli: {e}");
                continue;
            }
        };
        if let Err(e) = kubo.put_block(&cid, &block) {
            error!("Error sending block {cid} to kubo: {e}");
        }
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

    loop {
        if kubo.check_alive() && myceli.check_alive() {
            if let Err(e) = sync_blocks(&kubo, &myceli) {
                error!("Error during blocks sync: {e}");
            }
        }
        sleep(Duration::from_millis(cfg.sync_interval));
    }
}
