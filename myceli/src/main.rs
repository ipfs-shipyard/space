use anyhow::Result;
use clap::Parser;

use messages::DagInfo;
use myceli::config::Config;
use myceli::listener::Listener;
use std::collections::BTreeMap;
use std::net::ToSocketAddrs;

use std::sync::{Arc, Mutex};
use std::thread;
use tracing::Level;

#[derive(Parser, Debug)]
#[clap(about = "Myceli, a spacey IPFS node")]
struct Args {
    /// Path to config file
    config_path: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(Level::INFO)
        .init();

    let args = Args::parse();

    let cfg: Config = Config::parse(args.config_path).expect("Config parsing failed");

    let mut listener_handles = vec![];

    // These should probably eventually get extracted out into a non-persist in-memory db
    let nodes: Arc<Mutex<BTreeMap<String, Option<String>>>> = Arc::new(Mutex::new(BTreeMap::new()));
    let network_dags: Arc<Mutex<BTreeMap<String, DagInfo>>> = Arc::new(Mutex::new(BTreeMap::new()));

    nodes.lock().unwrap().insert(cfg.name.to_string(), None);

    for listener_cfg in cfg.listeners {
        let mut resolved_listen_addr = listener_cfg
            .address
            .to_socket_addrs()
            .expect("Unable to resolve socket address");
        let resolved_listen_addr = resolved_listen_addr
            .next()
            .expect("Unable to resolve socket addr");

        std::fs::create_dir_all(&cfg.storage_path).expect("Failed to create storage dir");

        let db_path = format!("{}/storage.db", cfg.storage_path);
        let thread_node_name = cfg.name.to_string();
        let thread_nodes = Arc::clone(&nodes);
        let thread_radio_address = cfg.radio_address.to_string();
        let thread_network_dags = Arc::clone(&network_dags);

        listener_handles.push(thread::spawn(move || {
            let mut listener = Listener::new(
                &resolved_listen_addr,
                &db_path,
                listener_cfg.mtu,
                &thread_node_name,
                thread_nodes,
                &thread_radio_address,
                thread_network_dags,
                listener_cfg.primary,
            )
            .expect("Listener creation failed");
            listener
                .start(listener_cfg.retry_timeout_duration)
                .expect("Error encountered in listener operation");
        }));
    }

    for handle in listener_handles {
        handle.join().unwrap();
    }

    Ok(())
}
