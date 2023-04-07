use crate::handlers;
use crate::shipper::Shipper;
use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::{ApplicationAPI, DataProtocol, Message, MessageChunker, SimpleChunker};
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use tracing::{debug, error, info};

pub struct Listener {
    storage_path: String,
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    chunker: SimpleChunker,
    socket: Arc<UdpSocket>,
    mtu: u16,
}

impl Listener {
    pub fn new(listen_address: &SocketAddr, storage_path: &str, mtu: u16) -> Result<Self> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        info!("Listening on {listen_address}");
        let socket = UdpSocket::bind(listen_address)?;
        Ok(Listener {
            storage_path: storage_path.to_string(),
            storage,
            sender_addr: None,
            chunker: SimpleChunker::new(mtu),
            socket: Arc::new(socket),
            mtu,
        })
    }

    pub fn start(&mut self, shipper_timeout_duration: u64) -> Result<()> {
        // First setup the shipper and its pieces
        let (shipper_sender, shipper_receiver) = mpsc::channel();
        let shipper_storage_path = self.storage_path.to_string();
        let shipper_sender_clone = shipper_sender.clone();
        let shipper_socket = Arc::clone(&self.socket);
        let shipper_mtu = self.mtu;
        spawn(move || {
            let mut shipper = Shipper::new(
                &shipper_storage_path,
                shipper_receiver,
                shipper_sender_clone,
                shipper_timeout_duration,
                shipper_socket,
                shipper_mtu,
            )
            .expect("Shipper creation failed");
            shipper.receive_msg_loop();
        });

        let mut buf = vec![0; usize::from(self.mtu)];
        loop {
            {
                loop {
                    match self.socket.recv_from(&mut buf) {
                        Ok((len, sender)) => {
                            if len > 0 {
                                self.sender_addr = Some(sender);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Recv failed {e}");
                        }
                    }
                    sleep(Duration::from_millis(10));
                }
            }

            match self.chunker.unchunk(&buf) {
                Ok(Some(msg)) => match self.handle_message(msg, shipper_sender.clone()) {
                    Ok(Some(resp)) => {
                        if let Err(e) = self.transmit_response(resp) {
                            error!("TransmitResponse error: {e}");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        if let Err(e) = self.transmit_response(Message::Error(e.to_string())) {
                            error!("TransmitResponse error: {e}");
                        }
                        error!("MessageHandlerError: {e}");
                    }
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

    fn handle_message(
        &mut self,
        message: Message,
        shipper_sender: Sender<(DataProtocol, String)>,
    ) -> Result<Option<Message>> {
        info!("Handling {message:?}");
        let resp = match message {
            Message::ApplicationAPI(ApplicationAPI::TransmitDag {
                cid,
                target_addr,
                retries,
            }) => {
                shipper_sender.send((
                    DataProtocol::RequestTransmitDag {
                        cid,
                        target_addr,
                        retries,
                    },
                    self.sender_addr
                        .map(|s| s.to_string())
                        .unwrap_or("".to_string()),
                ))?;

                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitBlock { cid, target_addr }) => {
                shipper_sender.send((
                    DataProtocol::RequestTransmitBlock { cid, target_addr },
                    self.sender_addr
                        .map(|s| s.to_string())
                        .unwrap_or("".to_string()),
                ))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path }) => {
                Some(handlers::import_file(&path, self.storage.clone())?)
            }
            Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path }) => {
                self.storage.export_cid(&cid, &PathBuf::from(path))?;
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
                shipper_sender.send((
                    data_msg,
                    self.sender_addr
                        .map(|s| s.to_string())
                        .unwrap_or("".to_string()),
                ))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestVersion) => {
                Some(Message::ApplicationAPI(ApplicationAPI::Version {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                }))
            }
            // Default case for valid messages which don't have handling code implemented yet
            message => {
                info!("Received unhandled message: {:?}", message);
                None
            }
        };
        Ok(resp)
    }

    fn transmit_msg(&self, message: Message, target_addr: SocketAddr) -> Result<()> {
        for chunk in self.chunker.chunk(message)? {
            self.socket.send_to(&chunk, target_addr)?;
        }
        Ok(())
    }

    fn transmit_response(&self, message: Message) -> Result<()> {
        if let Some(sender_addr) = self.sender_addr {
            self.transmit_msg(message, sender_addr)?;
        }
        Ok(())
    }
}
