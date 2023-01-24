use anyhow::Result;
use block_ship::chunking::path_to_chunks;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;
use tracing::info;

pub async fn transmit(path: &PathBuf, target_addr: &String) -> Result<()> {
    info!(
        "Transmitting {} in blocks to {}",
        path.display(),
        target_addr
    );
    let target_address: SocketAddr = target_addr.parse()?;
    let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
    let socket = UdpSocket::bind(&bind_address).await?;
    let data = path_to_chunks(path).await?;

    for packet in data {
        info!("Transmitting {} bytes", packet.len());
        socket.send_to(&packet, target_address).await?;
    }
    Ok(())
}
