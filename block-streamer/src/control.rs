use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use tracing::{error, info};

use crate::receive::receive;
use crate::receiver::Receiver;
use crate::transmit::{transmit, transmit_blocks};
use messages::chunking::{MessageChunker, SimpleChunker};
use messages::{ApplicationAPI, Message};

// TODO: Make this configurable
pub const MTU: u16 = 60; // 60 for radio

pub async fn control(listen_addr: &String) -> Result<()> {
    info!("Listening for messages on {}", listen_addr);
    let listen_address: SocketAddr = listen_addr.parse()?;

    // Setup storage
    let provider = SqliteStorageProvider::new("storage.db")?;
    provider.setup()?;
    let storage = Rc::new(Storage::new(Box::new(provider)));

    let mut buf = vec![0; usize::from(MTU)];
    let mut data_receiver = Receiver::new(Rc::clone(&storage));
    let mut sender_addr;

    let mut chunker = SimpleChunker::new(MTU);

    let socket = UdpSocket::bind(&listen_address).await?;

    loop {
        {
            loop {
                if let Ok((len, sender)) = socket.try_recv_from(&mut buf) {
                    if len > 0 {
                        sender_addr = Some(sender);
                        break;
                    }
                }
                sleep(Duration::from_millis(10)).await;
            }
        }

        match chunker.unchunk(&buf) {
            Ok(Message::ApplicationAPI(ApplicationAPI::Receive { listen_addr })) => {
                receive(&listen_addr).await?
            }
            Ok(Message::ApplicationAPI(ApplicationAPI::TransmitFile { path, target_addr })) => {
                transmit(&PathBuf::from(path), &target_addr).await?
            }
            Ok(Message::ApplicationAPI(ApplicationAPI::TransmitDag { cid, target_addr })) => {
                let root_block = storage.get_block_by_cid(&cid)?;
                let blocks = storage.get_all_blocks_under_cid(&cid)?;
                let mut all_blocks = vec![root_block];
                all_blocks.extend(blocks);
                transmit_blocks(&all_blocks, &target_addr).await?;
            }
            Ok(Message::ApplicationAPI(ApplicationAPI::ImportFile { path })) => {
                let root_cid = storage.import_path(&PathBuf::from(path.to_owned())).await?;
                let response = Message::ApplicationAPI(ApplicationAPI::FileImported {
                    path: path.to_string(),
                    cid: root_cid.to_string(),
                });
                if let Some(sender_addr) = sender_addr {
                    for chunk in chunker.chunk(response)? {
                        socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Ok(Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path })) => {
                storage.export_cid(&cid, &PathBuf::from(path)).await?
            }
            Ok(Message::ApplicationAPI(ApplicationAPI::RequestAvailableBlocks)) => {
                let raw_cids = storage.list_available_cids()?;
                let cids = raw_cids
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>();

                if let Some(sender_addr) = sender_addr {
                    let response =
                        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids });
                    for chunk in chunker.chunk(response)? {
                        socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Ok(Message::DataProtocol(data_msg)) => {
                data_receiver.handle_transmission_msg(data_msg).await?;
            }
            Ok(message) => {
                info!("Received unhandled message: {:?}", message);
            }
            Err(err) => {
                error!("{err}");
            }
        }
    }
}
