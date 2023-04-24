use crate::handlers;
use crate::shipper::Shipper;
use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::{ApplicationAPI, DataProtocol, Message};
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::spawn;
use tracing::{error, info};
use transports::Transport;

pub struct Listener {
    storage_path: String,
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    transport: Arc<dyn Transport + Send>,
}

impl Listener {
    pub fn new(
        listen_address: &SocketAddr,
        storage_path: &str,
        transport: Arc<dyn Transport + Send>,
    ) -> Result<Self> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        info!("Listening on {listen_address}");
        Ok(Listener {
            storage_path: storage_path.to_string(),
            storage,
            sender_addr: None,
            transport,
        })
    }

    pub fn start(&mut self, shipper_timeout_duration: u64) -> Result<()> {
        // First setup the shipper and its pieces
        let (shipper_sender, shipper_receiver) = mpsc::channel();
        let shipper_storage_path = self.storage_path.to_string();
        let shipper_sender_clone = shipper_sender.clone();
        let shipper_transport = Arc::clone(&self.transport);
        spawn(move || {
            let mut shipper = Shipper::new(
                &shipper_storage_path,
                shipper_receiver,
                shipper_sender_clone,
                shipper_timeout_duration,
                shipper_transport,
            )
            .expect("Shipper creation failed");
            shipper.receive_msg_loop();
        });

        loop {
            match self.transport.receive() {
                Ok((message, sender_addr)) => {
                    self.sender_addr = Some(sender_addr.to_socket_addrs()?.next().unwrap());
                    match self.handle_message(message, shipper_sender.clone()) {
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
                    }
                }
                Err(e) => {
                    error!("Receive message failed: {e}");
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
        self.transport.send(message, &target_addr.to_string())?;
        Ok(())
    }

    fn transmit_response(&self, message: Message) -> Result<()> {
        if let Some(sender_addr) = self.sender_addr {
            self.transmit_msg(message, sender_addr)?;
        }
        Ok(())
    }
}
