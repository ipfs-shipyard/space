use anyhow::Result;
use futures::TryStreamExt;
use iroh_resolver::unixfs_builder::{File, FileBuilder};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;

// use rand::seq::SliceRandom;
// use rand::thread_rng;

async fn chunk(path: &PathBuf) -> Result<Vec<Vec<u8>>> {
    // TODO: This clearly chunks up the file and
    // produces CIDs for each chunk, but this doesn't actually
    // create a DAG representing that file. I think I'm missing
    // a root block with the links to the sub blocks....which
    // might be the last non-raw block that I'm throwing out...or maybe not??
    let file: File = FileBuilder::new()
        .path(path)
        .fixed_chunker(24)
        .build()
        .await?;
    let parts: Vec<_> = file.encode().await?.try_collect().await?;

    let mut payloads = vec![];

    for part in parts {
        let mut payload = vec![];
        payload.extend(part.cid().to_bytes());
        payload.extend(part.data().to_vec());
        payloads.push(payload);
    }

    // TODO: This randomly shuffles the order of parts in the payload vec.
    // Once we have verification and reassembly working correctly on the receiver side,
    // we should be able to shuffle the payload and still get the correct file on the other side.
    // payloads.shuffle(&mut thread_rng());

    // TODO: The last part is always too big and contains what looks like a bunch of
    // garbage when I write it to the file...the current fix doesn't seem right haha
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
