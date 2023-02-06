use anyhow::Result;
use block_ship::chunking::{path_to_chunks, stored_block_to_chunks};
use local_storage::storage::StoredBlock;
use messages::Message;
use messages::TransmissionMessage;
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
        let msg = Message::DataProtocol(packet);
        let packet_bytes = msg.to_bytes();
        info!("Transmitting {} bytes", packet_bytes.len());
        socket.send_to(&packet_bytes, target_address).await?;
    }
    Ok(())
}

pub async fn transmit_blocks(blocks: &[StoredBlock], target_addr: &String) -> Result<()> {
    info!("Transmitting {} blocks to {}", blocks.len(), target_addr);

    let target_address: SocketAddr = target_addr.parse()?;
    let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
    let socket = UdpSocket::bind(&bind_address).await?;
    let data: Vec<TransmissionMessage> = blocks
        .iter()
        .map(|b| stored_block_to_chunks(b))
        // TODO: properly handle & log errors
        .filter_map(|list| list.ok())
        .flatten()
        .collect();

    for packet in data {
        let msg = Message::DataProtocol(packet);
        let packet_bytes = msg.to_bytes();
        info!("Transmitting {} bytes", packet_bytes.len());
        socket.send_to(&packet_bytes, target_address).await?;
    }

    Ok(())
}
