use crate::types::BlockWrapper;
use anyhow::Result;
use futures::TryStreamExt;
use iroh_unixfs::builder::{File, FileBuilder};
use parity_scale_codec::Encode;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;

async fn chunk(path: &PathBuf) -> Result<Vec<Vec<u8>>> {
    let file: File = FileBuilder::new()
        .path(path)
        .fixed_chunker(50)
        // This will decrease the width of the underlying tree
        // but the logic isn't ready on the receiving end end
        // and the current CID size means that two links will
        // still overrun the lab radio packet size
        // .degree(2)
        .build()
        .await?;

    let mut blocks: Vec<_> = file.encode().await?.try_collect().await?;

    let mut payloads = vec![];

    println!("{:?} broken into {} blocks", path.as_path(), blocks.len());

    blocks.shuffle(&mut thread_rng());

    for block in blocks {
        let wrapper = BlockWrapper::from_block(block)?;
        let chunks = wrapper.to_chunks()?;
        for c in chunks {
            payloads.push(c.encode());
        }

        // let blob = DataBlob::from_block(block)?;
        // if !blob.links.is_empty() {
        //     println!("{:?}", &blob);
        // }
        // payloads.push(blob.encode());
    }

    // This randomly shuffles the order of parts in the payload vec in order
    // to ensure reassembly is working correctly on the receiver side.
    // payloads.shuffle(&mut thread_rng());

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
        // sleep(Duration::from_millis(10));
    }
    Ok(())
}
