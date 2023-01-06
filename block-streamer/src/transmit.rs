use anyhow::Result;
use futures::TryStreamExt;
use iroh_resolver::unixfs_builder::{File, FileBuilder};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde_cbor_2::to_vec;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;

async fn chunk(path: &PathBuf) -> Result<Vec<Vec<u8>>> {
    let file: File = FileBuilder::new()
        .path(path)
        .fixed_chunker(24)
        .build()
        .await?;

    let blocks: Vec<_> = file.encode().await?.try_collect().await?;

    let mut payloads = vec![];

    for block in blocks {
        let mut datamap = BTreeMap::new();
        datamap.insert("c", vec![block.cid().to_bytes()]);
        if block.links().len() > 0 {
            // Still not sure what the data in the root block is...
            datamap.insert("d", vec![vec![]]);
        } else {
            datamap.insert("d", vec![block.data().to_vec()]);
        }

        let mut links = vec![];
        for l in block.links() {
            links.push(l.to_bytes());
        }
        datamap.insert("l", links);

        let buf = to_vec(&datamap)?;

        payloads.push(buf);
    }

    // This randomly shuffles the order of parts in the payload vec in order
    // to ensure reassembly is working correctly on the receiver side.
    payloads.shuffle(&mut thread_rng());

    Ok(payloads)
}

pub async fn transmit(path: &PathBuf, target_addr: &String) -> Result<()> {
    println!(
        "Transmitting {} in blocks to {}",
        path.display(),
        target_addr
    );
    let target_address: SocketAddr = target_addr.parse()?;
    let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
    let socket = UdpSocket::bind(&bind_address).await?;
    let data = chunk(path).await?;

    for packet in data {
        println!("Transmitting {} bytes", packet.len());
        socket.send_to(&packet, target_address).await?;
    }
    Ok(())
}
