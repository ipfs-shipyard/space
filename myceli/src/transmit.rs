use anyhow::Result;
use cid::Cid;
use local_storage::block::StoredBlock;
use messages::{DataProtocol, Message, MessageChunker, SimpleChunker, TransmissionBlock};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::info;

pub async fn transmit_blocks(blocks: &[StoredBlock], target_addr: &str) -> Result<()> {
    info!("Transmitting {} blocks to {}", blocks.len(), target_addr);

    let target_address: SocketAddr = target_addr.parse()?;
    let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
    let socket = UdpSocket::bind(&bind_address).await?;
    let chunker = SimpleChunker::new(crate::listener::MTU);

    for block in blocks {
        let mut links = vec![];
        for l in block.links.iter() {
            links.push(Cid::try_from(l.to_owned())?.to_bytes());
        }
        let block_cid = Cid::try_from(block.cid.to_owned())?;

        let transmission = TransmissionBlock {
            cid: block_cid.to_bytes(),
            data: block.data.to_vec(),
            links,
        };

        info!("Transmitting block {}", block_cid.to_string());
        let msg = Message::DataProtocol(DataProtocol::Block(transmission));
        for chunk in chunker.chunk(msg)? {
            socket.send_to(&chunk, target_address).await?;
        }
    }

    Ok(())
}
