use anyhow::Result;
use bytes::Bytes;
use cid::Cid;
use futures::TryStreamExt;
use iroh_resolver::resolver::Block;
use iroh_resolver::unixfs_builder::{File, FileBuilder};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Serialize, Serializer};
use serde_cbor_2::{to_vec, Value};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;

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

    let blocks: Vec<_> = file.encode().await?.try_collect().await?;

    // let root = file.encode_root().await?;

    // let parts = vec![root];

    let mut payloads = vec![];

    for block in blocks {
        // let mut payload = vec![];
        // payload.extend(block.cid().to_bytes());
        // payload.extend(block.data().to_vec());
        // payloads.push(payload);
        let mut datamap = BTreeMap::new();
        datamap.insert("cid", vec![block.cid().to_bytes()]);
        if block.links().len() > 0 {
            datamap.insert("data", vec![vec![]]);
        } else {
            datamap.insert("data", vec![block.data().to_vec()]);
        }

        let mut links = vec![];
        for l in block.links() {
            links.push(l.to_bytes());
        }
        datamap.insert("links", links);

        let buf = to_vec(&datamap).unwrap();
        // let mut encoder = Encoder::new(&mut buf[..]);
        // let res = encoder
        //     .begin_map()
        //     .unwrap()
        //     .bytes(&block.cid().to_bytes())
        //     .unwrap()
        //     .bytes(&block.data())
        //     .unwrap()
        //     .end()
        //     .unwrap();

        payloads.push(buf);
    }

    // This last part is the root block. I'm not sure what the part is
    // but I can see the links to the other CIDs
    // payloads.pop();

    // TODO: This randomly shuffles the order of parts in the payload vec.
    // Once we have verification and reassembly working correctly on the receiver side,
    // we should be able to shuffle the payload and still get the correct file on the other side.
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
