use crate::handlers;
use crate::shipper::Shipper;
use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::{ApplicationAPI, DataProtocol, Message};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::spawn;
use tracing::{debug, error, info};
use transports::Transport;

pub struct Listener<T> {
    storage_path: String,
    storage: Rc<Storage>,
    transport: Arc<T>,
    connected: bool,
}

impl<T: Transport + Send + 'static> Listener<T> {
    pub fn new(
        listen_address: &SocketAddr,
        storage_path: &str,
        transport: Arc<T>,
    ) -> Result<Listener<T>> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        info!("Listening on {listen_address}");
        Ok(Listener {
            storage_path: storage_path.to_string(),
            storage,
            transport,
            connected: true,
        })
    }

    pub fn start(&mut self, shipper_timeout_duration: u64, shipper_window_size: u8) -> Result<()> {
        // First setup the shipper and its pieces
        let (shipper_sender, shipper_receiver) = mpsc::channel();
        let shipper_storage_path = self.storage_path.to_string();
        let shipper_sender_clone = shipper_sender.clone();
        let shipper_transport = Arc::clone(&self.transport);
        let initial_connected = self.connected;
        spawn(move || {
            let mut shipper = Shipper::new(
                &shipper_storage_path,
                shipper_receiver,
                shipper_sender_clone,
                shipper_timeout_duration,
                shipper_window_size,
                shipper_transport,
                initial_connected,
            )
            .expect("Shipper creation failed");
            shipper.receive_msg_loop();
        });

        loop {
            match self.transport.receive() {
                Ok((message, sender_addr)) => {
                    match self.handle_message(message, &sender_addr, shipper_sender.clone()) {
                        Ok(Some(resp)) => {
                            if let Err(e) = self.transmit_response(resp, &sender_addr) {
                                error!("TransmitResponse error: {e}");
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            if let Err(e) =
                                self.transmit_response(Message::Error(e.to_string()), &sender_addr)
                            {
                                error!("TransmitResponse error: {e}");
                            }
                            error!("MessageHandlerError: {e}");
                        }
                    }
                }
                Err(e) => {
                    debug!("Receive message failed: {e}");
                }
            }
        }
    }

    fn handle_message(
        &mut self,
        message: Message,
        sender_addr: &str,
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
                    sender_addr.to_string(),
                ))?;

                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitBlock { cid, target_addr }) => {
                shipper_sender.send((
                    DataProtocol::RequestTransmitBlock { cid, target_addr },
                    sender_addr.to_string(),
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
                shipper_sender.send((data_msg, sender_addr.to_string()))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestVersion) => {
                Some(Message::ApplicationAPI(ApplicationAPI::Version {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected }) => {
                self.connected = connected;
                shipper_sender.send((
                    DataProtocol::SetConnected { connected },
                    sender_addr.to_string(),
                ))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::GetConnected) => {
                Some(Message::ApplicationAPI(ApplicationAPI::ConnectedState {
                    connected: self.connected,
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::ResumeTransmitDag { cid }) => {
                shipper_sender.send((
                    DataProtocol::ResumeTransmitDag { cid },
                    sender_addr.to_string(),
                ))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ResumeTransmitAllDags) => {
                shipper_sender
                    .send((DataProtocol::ResumeTransmitAllDags, sender_addr.to_string()))?;
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

    fn transmit_response(&self, message: Message, sender_addr: &str) -> Result<()> {
        self.transport.send(message, sender_addr)?;
        Ok(())
    }
}
