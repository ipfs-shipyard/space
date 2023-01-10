use crate::types::DataBlob;
use anyhow::Result;
use cid::Cid;
use iroh_unixfs::Block;
use parity_scale_codec::Decode;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;

async fn assemble(path: &PathBuf, root: &Block, blocks: &BTreeMap<Cid, Block>) -> Result<bool> {
    // First check if all CIDs exist
    for c in root.links().iter() {
        if !blocks.contains_key(c) {
            println!("Missing cid {}, wait for more data", c);
            return Ok(false);
        }
    }

    let mut output_file = File::create(path).await?;
    for cid in root.links().iter() {
        if let Some(data) = blocks.get(cid) {
            output_file.write_all(data.data()).await?;
        } else {
            // missing a cid...not ready yet...we shouldn't get
            // here because of the CIDs check above, but
            // we verify again anyways
            return Ok(false);
        }
    }
    output_file.flush().await?;

    Ok(true)
}

pub async fn receive(path: &PathBuf, listen_addr: &String) -> Result<()> {
    println!(
        "Listening for blocks of {} at {}",
        path.display(),
        listen_addr
    );
    let listen_address: SocketAddr = listen_addr.parse()?;
    let socket = UdpSocket::bind(&listen_address).await?;

    let mut buf = vec![0; 10240];

    let mut root: Option<Block> = None;
    let mut blocks: BTreeMap<Cid, Block> = BTreeMap::new();

    loop {
        println!(
            "Collected {} blocks, attempting to assemble file",
            blocks.len()
        );

        if let Some(root) = &root {
            if assemble(path, root, &blocks).await? {
                return Ok(());
            }
        }

        let mut receiving_cid = false;

        loop {
            if let Ok(len) = socket.try_recv(&mut buf) {
                if len > 0 {
                    let mut databuf = &buf[..len];
                    if let Ok(blob) = DataBlob::decode(&mut databuf) {
                        let block = blob.as_block()?;
                        // Check for root block
                        if !blob.links.is_empty() {
                            println!("Received root CID {}", &block.cid());
                            root = Some(block);
                        } else {
                            println!("Received child CID {} with {} bytes", &block.cid(), len);
                            blocks.insert(*block.cid(), block.clone());
                        }
                        receiving_cid = true;
                    }
                }
            } else if receiving_cid {
                break;
            }
            sleep(Duration::from_millis(10));
        }
    }
}
