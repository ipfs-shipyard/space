use anyhow::{anyhow, Result};
use cid::Cid;
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
}

impl DataBlob {
    pub fn try_parse(packet: &[u8]) -> Result<Self> {
        if packet.len() > 36 {
            let (cid, data) = packet.split_at(36);
            Ok(DataBlob {
                cid: Cid::try_from(cid)?,
                data: data.to_vec(),
            })
        } else {
            Err(anyhow!("Not enough data"))
        }
    }
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

    let mut buf = vec![0; 1024];
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

    // TODO: This is pretty dumb and provides no verification that these blocks belong together
    // or are in the correct order. First we probably need to transmit something closer to an
    // actual DAG structure with links, and then we can use that structure for verification
    // and reassembly here.
    let mut output_file = File::create(path).await?;
    for packet in data {
        output_file.write_all(&packet.data).await?;
    }
    output_file.flush().await?;

    Ok(())
}
