use crate::{handlers, shipper::Shipper, sync::Syncer};
use anyhow::Result;
use cid::Cid;
use local_storage::{provider::default_storage_provider, storage::Storage};
use log::{error, info, trace};
use messages::{ApplicationAPI, DataProtocol, Message};
use std::collections::BTreeSet;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::mpsc::{self, Sender},
    sync::{Arc, Mutex},
    thread::spawn,
};
use transports::{Transport, TransportError};

pub struct Listener<T> {
    storage: Storage,
    transport: Arc<T>,
    connected: Arc<Mutex<bool>>,
    radio_address: Option<String>,
    sync: Syncer,
    addrs: BTreeSet<String>,
}

impl<T: Transport + Send + 'static> Listener<T> {
    pub fn new(
        _listen_address: &SocketAddr,
        storage_path: &str,
        transport: Arc<T>,
        block_size: u32,
        radio_address: Option<String>,
        high_disk_usage: u64,
        mtu: u16,
    ) -> Result<Listener<T>> {
        let provider = default_storage_provider(storage_path, high_disk_usage)?;
        let missing_blocks = provider.lock().unwrap().get_dangling_cids()?;
        let storage = Storage::new(provider, block_size);
        let present_blocks = storage.list_available_cids()?;
        let present_blocks = present_blocks
            .iter()
            .flat_map(|s| Cid::try_from(s.as_str()));
        let sync = Syncer::new(mtu.into(), present_blocks, missing_blocks)?;
        info!("Listening on {_listen_address}");
        let addrs = if let Some(a) = &radio_address {
            BTreeSet::from([a.clone()])
        } else {
            BTreeSet::default()
        };
        Ok(Listener {
            storage,
            transport,
            connected: Arc::new(Mutex::new(true)),
            radio_address,
            sync,
            addrs,
        })
    }

    pub fn start(
        &mut self,
        shipper_timeout_duration: u64,
        shipper_window_size: u32,
        block_size: u32,
        shipper_packet_delay_ms: u32,
    ) -> Result<()> {
        // First setup the shipper and its pieces
        let (shipper_sender, shipper_receiver) = mpsc::channel();
        let shipper_sender_clone = shipper_sender.clone();
        let shipper_transport = Arc::clone(&self.transport);
        let initial_connected = Arc::clone(&self.connected);
        let shipper_radio = self.radio_address.clone();
        let shipper_storage_provider = self.storage.get_provider();
        spawn(move || {
            let mut shipper = Shipper::new(
                shipper_storage_provider,
                shipper_receiver,
                shipper_sender_clone,
                shipper_timeout_duration,
                shipper_window_size,
                shipper_transport,
                initial_connected,
                block_size,
                shipper_radio,
                shipper_packet_delay_ms
            )
            .expect("Shipper creation failed");
            shipper.receive_msg_loop();
        });

        loop {
            match self.transport.receive() {
                Ok((message, sender_addr)) => {
                    self.addrs.insert(sender_addr.clone());
                    let target_addr = if let Some(radio_address) = &self.radio_address {
                        radio_address.to_owned()
                    } else {
                        sender_addr.to_owned()
                    };
                    match self.handle_message(message, &target_addr, shipper_sender.clone()) {
                        Ok(Some(resp)) => {
                            if let Err(_e) = self.transmit_response(resp, &sender_addr) {
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
                Err(TransportError::TimedOut) => {
                    if let Err(e) = self.bg_tasks() {
                        error!("Error with background task: {e:?}");
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
        target: &str,
        shipper_sender: Sender<(DataProtocol, String)>,
    ) -> Result<Option<Message>> {
        trace!("Handling {message:?}");
        let resp: Option<Message> = match message {
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
                    target.to_string(),
                ))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitBlock { cid, target_addr }) => {
                shipper_sender.send((
                    DataProtocol::RequestTransmitBlock { cid, target_addr },
                    target.to_string(),
                ))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path }) => {
                Some(handlers::import_file(&path, &mut self.storage)?)
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
                Some(handlers::request_available_blocks(&self.storage)?)
            }
            Message::ApplicationAPI(ApplicationAPI::GetMissingDagBlocks { cid }) => {
                Some(handlers::get_missing_dag_blocks(&cid, &self.storage)?)
            }
            Message::ApplicationAPI(ApplicationAPI::ValidateDag { cid }) => {
                Some(handlers::validate_dag(&cid, &self.storage)?)
            }
            Message::DataProtocol(data_msg) => {
                shipper_sender.send((data_msg, target.to_string()))?;
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
                        .send((DataProtocol::ResumeTransmitAllDags, target.to_string()))?;
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::GetConnected) => {
                Some(Message::ApplicationAPI(ApplicationAPI::ConnectedState {
                    connected: *self.connected.lock().unwrap(),
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::ResumeTransmitDag { cid }) => {
                shipper_sender
                    .send((DataProtocol::ResumeTransmitDag { cid }, target.to_string()))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ResumeTransmitAllDags) => {
                shipper_sender.send((DataProtocol::ResumeTransmitAllDags, target.to_string()))?;
                None
            }
            #[allow(unused_variables)]
            Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse { cid, result }) => {
                info!("Received ValidateDagResponse from {target} for {cid}: {result}");
                None
            }
            #[allow(unused_variables)]
            Message::ApplicationAPI(ApplicationAPI::FileImported { path, cid }) => {
                info!("Received FileImported from {target}: {path} -> {cid}");
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
                Some(handlers::get_available_dags(&self.storage)?)
            }
            Message::Sync(sm) => self.sync.handle(sm, &mut self.storage)?,
            // Default case for valid messages which don't have handling code implemented yet
            message => {
                info!("Received message: {:?}", message);
                None
            }
        };
        Ok(resp)
    }

    fn transmit_response(&self, message: Message, target_addr: &str) -> Result<()> {
        self.transport.send(message, target_addr)?;
        Ok(())
    }

    fn bg_tasks(&mut self) -> Result<()> {
        if !self.addrs.is_empty() {
            if let Some(msg) = self.sync.pop_pending_msg() {
                for addr in &self.addrs {
                    self.transport.send(msg.clone(), addr)?;
                }
                return Ok(());
            }
        }
        if !self.storage.incremental_gc() {
            self.sync.build_msg()?;
        }
        Ok(())
    }
}
