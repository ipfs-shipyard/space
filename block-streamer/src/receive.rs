use anyhow::Result;
use block_ship::chunking::chunks_to_path;
use block_ship::types::{BlockWrapper, TransmissionChunk, TransmissionMessage};
use cid::Cid;
use iroh_unixfs::Block;
use parity_scale_codec::Decode;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::info;

pub async fn receive(path: &PathBuf, listen_addr: &String) -> Result<()> {
    info!(
        "Listening for blocks of {} at {}",
        path.display(),
        listen_addr
    );
    let listen_address: SocketAddr = listen_addr.parse()?;
    let socket = UdpSocket::bind(&listen_address).await?;

    let mut buf = vec![0; 10240];

    let mut root: Option<Block> = None;
    let mut blocks: BTreeMap<Cid, Block> = BTreeMap::new();
    let mut current_cid: Option<Vec<u8>> = None;
    let mut current_cid_chunks: Vec<TransmissionChunk> = vec![];

    loop {
        info!(
            "Collected {} blocks, attempting to assemble file",
            blocks.len()
        );

        info!(
            "Current CID {:?} has {} chunks",
            current_cid,
            current_cid_chunks.len()
        );

        if let Some(root) = &root {
            info!("Found root, try to assemble");
            if chunks_to_path(path, root, &blocks).await? {
                info!("Assembly success!!");
                return Ok(());
            }
        }

        let mut receiving_cid = false;

        loop {
            if let Ok(len) = socket.try_recv(&mut buf) {
                if len > 0 {
                    let mut databuf = &buf[..len];
                    // TODO: Some of this reassembly logic needs to be extracted so that it can be tested directly
                    // TODO: This also should be reworked to handle a stream of chunks/blocks sent totally out of order
                    if let Ok(message) = TransmissionMessage::decode(&mut databuf) {
                        info!("Received Message {} bytes", len);
                        match message {
                            TransmissionMessage::Cid(cid) => {
                                info!("Received CID: {:?}", cid);
                                current_cid = Some(cid);
                                current_cid_chunks.clear();
                            }
                            TransmissionMessage::Chunk(chunk) => {
                                info!("Received Chunk for CID: {:?}", chunk.cid_marker);
                                if let Some(current_cid) = &current_cid {
                                    current_cid_chunks.push(chunk);

                                    if let Ok(wrapper) =
                                        BlockWrapper::from_chunks(current_cid, &current_cid_chunks)
                                    {
                                        let block = wrapper.to_block()?;
                                        if !block.links().is_empty() {
                                            info!("Found root CID {}", &block.cid());
                                            root = Some(block);
                                        } else {
                                            info!(
                                                "Found child CID {} with {} bytes",
                                                &block.cid(),
                                                len
                                            );
                                            blocks.insert(*block.cid(), block.clone());
                                        }
                                        receiving_cid = true;
                                    }
                                }
                            }
                        }
                    }
                }
            } else if receiving_cid {
                break;
            }
            sleep(Duration::from_millis(10));
        }
    }
}
