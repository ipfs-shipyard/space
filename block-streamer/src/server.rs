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

use crate::handlers;
use crate::receiver::Receiver;
use messages::{ApplicationAPI, Message, MessageChunker, SimpleChunker};

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
                Ok(Some(msg)) => match self.handle_message(msg).await {
                    Ok(Some(resp)) => self.transmit_response(resp).await.unwrap(),
                    Ok(None) => {}
                    Err(e) => error!("{e}"),
                },
                Ok(None) => {
                    debug!("No msg found yet");
                }
                Err(err) => {
                    error!("Message parse failed: {err}");
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Message) -> Result<Option<Message>> {
        let resp = match message {
            Message::ApplicationAPI(ApplicationAPI::TransmitFile { path, target_addr }) => {
                handlers::transmit_file(&path, &target_addr, self.storage.clone()).await?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitDag { cid, target_addr }) => {
                handlers::transmit_dag(&cid, &target_addr, self.storage.clone()).await?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path }) => {
                Some(handlers::import_file(&path, self.storage.clone()).await?)
            }
            Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path }) => {
                self.storage.export_cid(&cid, &PathBuf::from(path)).await?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestAvailableBlocks) => {
                Some(handlers::request_available_blocks(self.storage.clone())?)
            }
            Message::ApplicationAPI(ApplicationAPI::GetMissingDagBlocks { cid }) => Some(
                handlers::get_missing_dag_blocks(&cid, self.storage.clone())?,
            ),
            Message::ApplicationAPI(ApplicationAPI::ValidateDag { cid }) => {
                Some(handlers::validate_dag(&cid, self.storage.clone())?)
            }
            Message::DataProtocol(data_msg) => {
                self.receiver.handle_transmission_msg(data_msg).await?;
                None
            }
            // Default case for valid messages which don't have handling code implemented yet
            message => {
                info!("Received unhandled message: {:?}", message);
                None
            }
        };
        Ok(resp)
    }

    async fn transmit_response(&self, message: Message) -> Result<()> {
        if let Some(sender_addr) = self.sender_addr {
            for chunk in self.chunker.chunk(message)? {
                self.socket.send_to(&chunk, sender_addr).await?;
            }
        }
        Ok(())
    }
}
