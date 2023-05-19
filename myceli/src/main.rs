use anyhow::Result;
use clap::Parser;
use myceli::config::Config;
use myceli::listener::Listener;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tracing::Level;
use transports::UdpTransport;

#[derive(Parser, Debug)]
#[clap(about = "Myceli, a spacey IPFS node")]
struct Args {
    /// Path to config file
    config_path: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();

    let cfg = Config::parse(args.config_path).expect("Failed to parse config");

    let mut resolved_listen_addr = cfg
        .listen_address
        .to_socket_addrs()
        .expect("Unable to resolve socket address");
    let resolved_listen_addr = resolved_listen_addr
        .next()
        .expect("Unable to resolve socket addr");

    std::fs::create_dir_all(&cfg.storage_path).expect("Failed to create storage dir");

    let db_path = format!("{}/storage.db", cfg.storage_path);

    let udp_transport =
        UdpTransport::new(&cfg.listen_address, cfg.mtu).expect("Failed to create udp transport");

    let mut listener = Listener::new(
        &resolved_listen_addr,
        &db_path,
        Arc::new(udp_transport),
        cfg.block_size,
    )
    .expect("Listener creation failed");
    listener
        .start(cfg.retry_timeout_duration, cfg.window_size, cfg.block_size)
        .expect("Error encountered in listener operation");
    Ok(())
}
