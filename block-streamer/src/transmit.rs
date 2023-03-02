use anyhow::Result;
use cid::Cid;
use local_storage::block::StoredBlock;
use messages::{DataProtocol, Message, MessageChunker, SimpleChunker, TransmissionBlock};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::info;

pub async fn transmit_blocks(blocks: &[StoredBlock], target_addr: &String) -> Result<()> {
    info!("Transmitting {} blocks to {}", blocks.len(), target_addr);

    let target_address: SocketAddr = target_addr.parse()?;
    let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
    let socket = UdpSocket::bind(&bind_address).await?;
    let chunker = SimpleChunker::new(crate::server::MTU);

    let data: Vec<TransmissionBlock> = blocks
        .iter()
        .map(|block| {
            let mut links = vec![];
            for l in block.links.iter() {
                links.push(Cid::try_from(l.to_owned()).unwrap().to_bytes());
            }

            // Right now we're ignoring the data attached to the root nodes
            // because the current assembly method doesn't require it
            // and it saves a decent amount of payload weight
            let data = if !links.is_empty() {
                vec![]
            } else {
                block.data.to_vec()
            };
            TransmissionBlock {
                cid: Cid::try_from(block.cid.to_owned()).unwrap().to_bytes(),
                data,
                links,
            }
        })
        .collect();

    for block in data {
        info!(
            "Transmitting block {}",
            Cid::try_from(block.cid.clone()).unwrap().to_string()
        );
        let msg = Message::DataProtocol(DataProtocol::Block(block));
        for chunk in chunker.chunk(msg)? {
            socket.send_to(&chunk, target_address).await?;
        }
    }

    Ok(())
}
