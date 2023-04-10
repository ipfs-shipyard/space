use anyhow::Result;
use clap::Parser;
use myceli::config::Config;
use myceli::listener::Listener;
use std::net::ToSocketAddrs;
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

        listener_handles.push(thread::spawn(move || {
            let mut listener = Listener::new(&resolved_listen_addr, &db_path, listener_cfg.mtu)
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
