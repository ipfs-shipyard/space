use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use tracing::{debug, error, info};

use crate::receive::receive;
use crate::receiver::Receiver;
use crate::transmit::transmit_blocks;
use messages::chunking::{MessageChunker, SimpleChunker};
use messages::{ApplicationAPI, Message};

// TODO: Make this configurable
pub const MTU: u16 = 60; // 60 for radio

pub struct Server {
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    chunker: SimpleChunker,
    receiver: Receiver,
    socket: Rc<UdpSocket>,
}

impl Server {
    pub async fn new(listen_address: &str) -> Result<Self> {
        let provider = SqliteStorageProvider::new("storage.db")?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        let socket = UdpSocket::bind(&listen_address).await?;
        info!("Listening for messages on {}", listen_address);
        let receiver = Receiver::new(Rc::clone(&storage));
        Ok(Server {
            storage,
            sender_addr: None,
            chunker: SimpleChunker::new(MTU),
            receiver,
            socket: Rc::new(socket),
        })
    }

    pub async fn listen(&mut self) -> Result<()> {
        let mut buf = vec![0; usize::from(MTU)];
        loop {
            {
                loop {
                    if let Ok((len, sender)) = self.socket.try_recv_from(&mut buf) {
                        if len > 0 {
                            self.sender_addr = Some(sender);
                            break;
                        }
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }

            match self.chunker.unchunk(&buf) {
                Ok(Some(msg)) => {
                    if let Err(e) = self.handle_message(msg).await {
                        error!("{e}");
                    }
                }
                Ok(None) => {
                    debug!("No msg found yet");
                }
                Err(err) => {
                    error!("{err}");
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Message) -> Result<()> {
        Ok(match message {
            Message::ApplicationAPI(ApplicationAPI::Receive { listen_addr }) => {
                receive(&listen_addr).await?
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitFile { path, target_addr }) => {
                let cid = self
                    .storage
                    .import_path(&PathBuf::from(path.to_owned()))
                    .await?;
                let root_block = self.storage.get_block_by_cid(&cid)?;
                let blocks = self.storage.get_all_blocks_under_cid(&cid)?;
                let mut all_blocks = vec![root_block];
                all_blocks.extend(blocks);
                transmit_blocks(&all_blocks, &target_addr).await?
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitDag { cid, target_addr }) => {
                let root_block = self.storage.get_block_by_cid(&cid)?;
                let blocks = self.storage.get_all_blocks_under_cid(&cid)?;
                let mut all_blocks = vec![root_block];
                all_blocks.extend(blocks);
                transmit_blocks(&all_blocks, &target_addr).await?
            }
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path }) => {
                let root_cid = self
                    .storage
                    .import_path(&PathBuf::from(path.to_owned()))
                    .await?;
                let response = Message::ApplicationAPI(ApplicationAPI::FileImported {
                    path: path.to_string(),
                    cid: root_cid.to_string(),
                });
                if let Some(sender_addr) = self.sender_addr {
                    for chunk in self.chunker.chunk(response)? {
                        self.socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path }) => {
                self.storage.export_cid(&cid, &PathBuf::from(path)).await?
            }
            Message::ApplicationAPI(ApplicationAPI::RequestAvailableBlocks) => {
                let raw_cids = self.storage.list_available_cids()?;
                let cids = raw_cids
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>();

                if let Some(sender_addr) = self.sender_addr {
                    let response =
                        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids });
                    for chunk in self.chunker.chunk(response)? {
                        self.socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Message::ApplicationAPI(ApplicationAPI::GetMissingDagBlocks { cid }) => {
                let missing_blocks = self.storage.get_missing_dag_blocks(&cid)?;
                if let Some(sender_addr) = self.sender_addr {
                    let response = Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks {
                        blocks: missing_blocks,
                    });
                    for chunk in self.chunker.chunk(response)? {
                        self.socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Message::DataProtocol(data_msg) => {
                self.receiver.handle_transmission_msg(data_msg).await?;
            }
            message => {
                info!("Received unhandled message: {:?}", message);
            }
        })
    }
}
