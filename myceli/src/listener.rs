use crate::handlers;
use crate::shipper::Shipper;
use anyhow::Result;
use local_storage::{provider::default_storage_provider, storage::Storage};
use messages::{ApplicationAPI, DataProtocol, Message};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use transports::Transport;

use log::{error, info};

pub struct Listener<T> {
    storage_path: String,
    storage: Rc<Storage>,
    transport: Arc<T>,
    connected: Arc<Mutex<bool>>,
    radio_address: Option<String>,
}

impl<T: Transport + Send + 'static> Listener<T> {
    pub fn new(
        _listen_address: &SocketAddr,
        storage_path: &str,
        transport: Arc<T>,
        block_size: u32,
        radio_address: Option<String>,
    ) -> Result<Listener<T>> {
        let storage = Rc::new(Storage::new(
            default_storage_provider(storage_path)?,
            block_size,
        ));

        info!("Listening on {_listen_address}");
        Ok(Listener {
            storage_path: storage_path.to_string(),
            storage,
            transport,
            connected: Arc::new(Mutex::new(true)),
            radio_address,
        })
    }

    pub fn start(
        &mut self,
        shipper_timeout_duration: u64,
        shipper_window_size: u32,
        block_size: u32,
    ) -> Result<()> {
        // First setup the shipper and its pieces
        let (shipper_sender, shipper_receiver) = mpsc::channel();
        let shipper_storage_path = self.storage_path.to_string();
        let shipper_sender_clone = shipper_sender.clone();
        let shipper_transport = Arc::clone(&self.transport);
        let initial_connected = Arc::clone(&self.connected);
        let shipper_radio = self.radio_address.clone();
        spawn(move || {
            let mut shipper = Shipper::new(
                &shipper_storage_path,
                shipper_receiver,
                shipper_sender_clone,
                shipper_timeout_duration,
                shipper_window_size,
                shipper_transport,
                initial_connected,
                block_size,
                shipper_radio,
            )
            .expect("Shipper creation failed");
            shipper.receive_msg_loop();
        });

        loop {
            match self.transport.receive() {
                Ok((message, sender_addr)) => {
                    let target_addr = if let Some(radio_address) = &self.radio_address {
                        radio_address.to_owned()
                    } else {
                        sender_addr.to_owned()
                    };
                    match self.handle_message(message, &target_addr, shipper_sender.clone()) {
                        Ok(Some(resp)) => {
                            if let Err(_e) = self.transmit_response(resp, &target_addr) {
                                error!("TransmitResponse error: {_e}");
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            if let Err(_e) =
                                self.transmit_response(Message::Error(e.to_string()), &target_addr)
                            {
                                error!("TransmitResponse error: {_e}");
                            }

                            error!("MessageHandlerError: {e}");
                        }
                    }
                }
                Err(_e) => {
                    println!("Receive message failed: {_e}");
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
        println!("Handling {message:?}");
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
                match self.storage.export_cid(&cid, &PathBuf::from(path.clone())) {
                    Ok(()) => Some(Message::ApplicationAPI(ApplicationAPI::DagExported {
                        cid,
                        path,
                    })),
                    Err(e) => Some(Message::ApplicationAPI(ApplicationAPI::DagExportFailed {
                        cid,
                        path,
                        error: e.to_string(),
                    })),
                }
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
                let prev_connected = *self.connected.lock().unwrap();
                *self.connected.lock().unwrap() = connected;
                if !prev_connected && connected {
                    shipper_sender
                        .send((DataProtocol::ResumeTransmitAllDags, sender_addr.to_string()))?;
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::GetConnected) => {
                Some(Message::ApplicationAPI(ApplicationAPI::ConnectedState {
                    connected: *self.connected.lock().unwrap(),
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
            #[allow(unused_variables)]
            Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse { cid, result }) => {
                info!("Received ValidateDagResponse from {sender_addr} for {cid}: {result}");
                None
            }
            #[allow(unused_variables)]
            Message::ApplicationAPI(ApplicationAPI::FileImported { path, cid }) => {
                info!("Received FileImported from {sender_addr}: {path} -> {cid}");
                None
            }
            Message::ApplicationAPI(ApplicationAPI::DagTransmissionComplete { cid }) => {
                let dag_blocks = self.storage.get_all_dag_blocks(&cid)?;
                match local_storage::block::validate_dag(&dag_blocks) {
                    Ok(_) => {
                        info!("Sucessfully received and validated dag {cid}")
                    }
                    Err(_e) => {
                        error!("Failure in receiving dag {cid}: {}", _e.to_string());
                        // TODO: Delete dag and restart transmission at this point?
                    }
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestAvailableDags) => {
                Some(handlers::get_available_dags(self.storage.clone())?)
            }
            // Default case for valid messages which don't have handling code implemented yet
            _message => {
                info!("Received message: {:?}", _message);
                None
            }
        };
        Ok(resp)
    }

    fn transmit_response(&self, message: Message, target_addr: &str) -> Result<()> {
        self.transport.send(message, target_addr)?;
        Ok(())
    }
}
