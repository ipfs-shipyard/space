use anyhow::{anyhow, Result};
use cid::Cid;
use serde_cbor_2::Deserializer;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;

#[derive(Clone)]
struct DataBlob {
    pub cid: Cid,
    pub data: Vec<u8>,
    pub links: Vec<Cid>,
}

impl DataBlob {
    pub fn try_parse(packet: &[u8]) -> Result<Self> {
        if packet.len() > 36 {
            let mut deser = Deserializer::from_slice(&packet);
            let datamap: BTreeMap<&str, Vec<Vec<u8>>> =
                serde::de::Deserialize::deserialize(&mut deser)?;
            let cid = datamap
                .get("c")
                .ok_or(anyhow!("Failed to find cid"))?
                .clone()
                .pop()
                .ok_or(anyhow!("Found malformed cid"))?;
            let mut data = datamap
                .get("d")
                .ok_or(anyhow!("Failed to find data"))?
                .clone();
            let data = data.pop().ok_or(anyhow!("Found malformed data"))?;
            let raw_links = datamap
                .get("l")
                .ok_or(anyhow!("Failed to find links"))?
                .clone();

            let mut links = vec![];
            for link in raw_links {
                links.push(Cid::try_from(link)?);
            }

            Ok(DataBlob {
                cid: Cid::try_from(cid)?,
                data,
                links,
            })
        } else {
            Err(anyhow!("Not enough data"))
        }
    }
}

async fn assemble(
    path: &PathBuf,
    root: &DataBlob,
    blocks: &BTreeMap<Cid, DataBlob>,
) -> Result<bool> {
    // First check if all cids exist
    for c in &root.links {
        if !blocks.contains_key(&c) {
            println!("Missing cid {}, wait for more data", c);
            return Ok(false);
        }
    }

    let mut output_file = File::create(path).await?;
    for cid in &root.links {
        if let Some(data) = blocks.get(&cid) {
            output_file.write_all(&data.data).await?;
        } else {
            // missing a cid...not ready yet
            return Ok(false);
        }
    }
    output_file.flush().await?;

    return Ok(true);
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

    let mut root: Option<DataBlob> = None;
    let mut blocks: BTreeMap<Cid, DataBlob> = BTreeMap::new();

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
                    // In try_parse we verify we received a valid CID...that is probably
                    // good enough verification for here
                    if let Ok(blob) = DataBlob::try_parse(&buf[..len]) {
                        println!("Received CID {} with {} bytes", &blob.cid, len);
                        // Check for root block
                        if blob.links.len() > 0 {
                            root = Some(blob);
                        } else {
                            blocks.insert(blob.cid, blob);
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
