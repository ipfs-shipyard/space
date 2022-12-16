use anyhow::Result;
use cid::Cid;
use futures::TryStreamExt;
use iroh_resolver::unixfs_builder::{File, FileBuilder};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;

// use rand::seq::SliceRandom;
// use rand::thread_rng;

async fn chunk(path: &PathBuf) -> Result<Vec<Vec<u8>>> {
    let file: File = FileBuilder::new()
        .path(path)
        .fixed_chunker(24)
        .build()
        .await?;

    let _root: Option<Cid> = None;
    let parts: Vec<_> = file.encode().await?.try_collect().await?;

    let mut payloads = vec![];

    for part in parts {
        let mut payload = vec![];
        payload.extend(part.cid().to_bytes());
        payload.extend(part.data().to_vec());
        payloads.push(payload);
    }

    // payloads.shuffle(&mut thread_rng());

    payloads.pop();

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
