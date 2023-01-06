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

struct DataBlob {
    pub cid: Cid,
    pub data: Vec<u8>,
    pub links: Vec<Cid>,
}

impl DataBlob {
    pub fn try_parse(packet: &[u8]) -> Result<Self> {
        if packet.len() > 36 {
            // let mut decoder = Decoder::new(&packet);
            // let map = decoder.map().unwrap();
            // let cid = decoder.bytes().unwrap();
            // let data = decoder.bytes().unwrap();
            let mut deser = Deserializer::from_slice(&packet);
            let datamap: BTreeMap<&str, Vec<Vec<u8>>> =
                serde::de::Deserialize::deserialize(&mut deser).unwrap();
            let cid = datamap.get("cid").unwrap().clone().pop().unwrap();
            let data = datamap.get("data").unwrap();
            let raw_links = datamap.get("links").unwrap().clone();

            let mut links = vec![];
            for link in raw_links {
                links.push(Cid::try_from(link)?);
            }

            // let (cid, data) = packet.split_at(36);
            Ok(DataBlob {
                cid: Cid::try_from(cid)?,
                data: data.first().unwrap().to_vec(),
                links,
            })
        } else {
            Err(anyhow!("Not enough data"))
        }
    }
}

async fn assemble(path: &PathBuf, blocks: Vec<DataBlob>) -> Result<()> {
    // TODO: This is pretty dumb and provides no verification that these blocks belong together
    // or are in the correct order. First we probably need to transmit something closer to an
    // actual DAG structure with links, and then we can use that structure for verification
    // and reassembly here.

    let root = blocks.iter().find(|b| b.links.len() > 0);

    if let Some(root) = root {
        let mut output_file = File::create(path).await?;
        for cid in &root.links {
            if let Some(data) = blocks.iter().find(|b| b.cid == *cid) {
                output_file.write_all(&data.data).await?;
            }
        }
        output_file.flush().await?;
    }

    Ok(())

    // for packet in blocks {
    //     output_file.write_all(&packet.data).await?;
    // }
}

pub async fn receive(path: &PathBuf, listen_addr: &String) -> Result<()> {
    println!(
        "Listening for blocks of {} at {}",
        path.display(),
        listen_addr
    );
    let listen_address: SocketAddr = listen_addr.parse()?;
    let socket = UdpSocket::bind(&listen_address).await?;

    let mut data = vec![];

    let mut buf = vec![0; 10240];
    let mut receiving_cid = false;
    loop {
        if let Ok(len) = socket.try_recv(&mut buf) {
            if len > 0 {
                // In try_parse we verify we received a valid CID...that is probably
                // good enough verification for here
                if let Ok(blob) = DataBlob::try_parse(&buf[..len]) {
                    println!("Received CID {} with {} bytes", &blob.cid, len);
                    data.push(blob);
                    receiving_cid = true;
                }
            }
        } else if receiving_cid {
            break;
        }
        sleep(Duration::from_millis(10));
    }

    println!("Received {} packets, writing file", data.len());

    Ok(assemble(path, data).await.expect("File assembly failed"))
}
