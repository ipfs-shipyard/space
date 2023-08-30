use anyhow::Result;
use config::Config;
use myceli::listener::Listener;
use std::{net::ToSocketAddrs, path::PathBuf, str::FromStr, sync::Arc, time::Duration};
use transports::UdpTransport;

#[cfg(all(not(feature = "sqlite"), not(feature = "files")))]
compile_error! {"Myceli built without a local storage implementation will not function. Select a feature, recommended: either big or small"}

fn main() -> Result<()> {
    #[cfg(feature = "good_log")]
    env_logger::init();
    #[cfg(feature = "small_log")]
    smalog::init();
    let config_path = std::env::args()
        .skip(1)
        .find(|a| PathBuf::from_str(a).map(|p| p.is_file()).unwrap_or(false));
    let cfg = Config::parse(config_path).expect("Failed to parse config");
    if std::env::args().any(|a| a == "--show-config") {
        println!("{}", toml::to_string(&cfg).unwrap());
        return Ok(());
    }

    let mut resolved_listen_addr = cfg
        .listen_address
        .to_socket_addrs()
        .expect("Unable to resolve socket address");
    let resolved_listen_addr = resolved_listen_addr
        .next()
        .expect("Unable to resolve socket addr");

    std::fs::create_dir_all(&cfg.storage_path).expect("Failed to create storage dir");

    let db_path = cfg.storage_path.clone();
    let disk_bytes = cfg.disk_usage * 1024;
    let timeout = if cfg.gc_period_ms > 0 {
        Some(Duration::from_millis(cfg.gc_period_ms.into()))
    } else {
        None
    };
    let mut udp_transport =
        UdpTransport::new(&cfg.listen_address, cfg.mtu, cfg.chunk_transmit_throttle)
            .expect("Failed to create udp transport");
    udp_transport
        .set_read_timeout(timeout)
        .expect("Failed to set timeout");
    println!("pid={}", std::process::id());
    let mut listener = Listener::new(
        &resolved_listen_addr,
        &db_path,
        Arc::new(udp_transport),
        cfg.block_size
            .expect("Block size default should've been calculated."),
        cfg.radio_address,
        disk_bytes,
        cfg.mtu,
    )
    .expect("Listener creation failed");
    listener
        .start(
            cfg.retry_timeout_duration,
            cfg.window_size,
            cfg.block_size.unwrap(),
        )
        .expect("Error encountered in listener operation");
    Ok(())
}
