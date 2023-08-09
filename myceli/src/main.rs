use anyhow::Result;
use config::Config;
use myceli::listener::Listener;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use transports::UdpTransport;

#[cfg(all(not(feature = "small"), not(feature = "big")))]
compile_error! {"Select either big or small feature"}

fn main() -> Result<()> {
    #[cfg(feature = "good_log")]
    env_logger::init();
    #[cfg(feature = "small_log")]
    smalog::init();

    let config_path = std::env::args().nth(1);
    let cfg = Config::parse(config_path).expect("Failed to parse config");

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
        UdpTransport::new(&cfg.listen_address, cfg.mtu, cfg.chunk_transmit_throttle)
            .expect("Failed to create udp transport");
    println!("pid={}", std::process::id());
    let mut listener = Listener::new(
        &resolved_listen_addr,
        &db_path,
        Arc::new(udp_transport),
        cfg.block_size,
        cfg.radio_address,
    )
    .expect("Listener creation failed");
    listener
        .start(cfg.retry_timeout_duration, cfg.window_size, cfg.block_size)
        .expect("Error encountered in listener operation");
    Ok(())
}
