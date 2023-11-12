use crate::handlers;
#[cfg(feature = "proto_ship")]
use crate::shipper::Shipper;
#[cfg(feature = "proto_sync")]
use crate::sync::Syncer;
use anyhow::Result;
use local_storage::{provider::default_storage_provider, storage::Storage};
use log::{debug, error, info, trace};
use messages::{ApplicationAPI, DataProtocol, Message, SyncMessage};
use std::collections::BTreeSet;
#[cfg(feature = "proto_ship")]
use std::thread::spawn;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::mpsc::{self, Sender},
    sync::{Arc, Mutex},
};
use transports::{Transport, TransportError};

pub struct Listener<T> {
    storage: Storage,
    transport: Arc<T>,
    connected: Arc<Mutex<bool>>,
    radio_address: Option<String>,
    addrs: BTreeSet<String>,
    sync_counts: [u64; 3],
    _block_size: u32,
    #[cfg(feature = "proto_sync")]
    sync: Syncer,
}

impl<T: Transport + Send + 'static> Listener<T> {
    pub fn new(
        listen_address: &SocketAddr,
        storage_path: &str,
        transport: Arc<T>,
        block_size: u32,
        radio_address: Option<String>,
        high_disk_usage: u64,
        _mtu: u16,
    ) -> Result<Listener<T>> {
        let provider = default_storage_provider(storage_path, high_disk_usage)?;
        let storage = Storage::new(provider, block_size);
        info!("Listening on {listen_address}");
        let addrs = if let Some(a) = &radio_address {
            BTreeSet::from([a.clone()])
        } else {
            BTreeSet::default()
        };
        #[cfg(feature = "proto_sync")]
        let sync = create_syncer(&storage, _mtu)?;
        Ok(Listener {
            storage,
            transport,
            connected: Arc::new(Mutex::new(true)),
            radio_address,
            addrs,
            sync_counts: [0; 3],
            _block_size: block_size,
            #[cfg(feature = "proto_sync")]
            sync,
        })
    }

    pub fn start(
        &mut self,
        _shipper_timeout_duration: u64,
        _shipper_window_size: u32,
        _shipper_packet_delay_ms: u32,
    ) -> Result<()> {
        let (shipper_sender, _shipper_receiver) = mpsc::channel();
        #[cfg(feature = "proto_ship")]
        {
            let initial_connected = Arc::clone(&self.connected);
            // First setup the shipper and its pieces
            let shipper_transport = Arc::clone(&self.transport);
            let shipper_radio = self.radio_address.clone();
            let shipper_storage_provider = self.storage.get_provider();
            let shipper_sender_clone = shipper_sender.clone();
            let block_size = self._block_size;
            spawn(move || {
                let mut shipper = Shipper::new(
                    shipper_storage_provider,
                    _shipper_receiver,
                    shipper_sender_clone,
                    _shipper_timeout_duration,
                    _shipper_window_size,
                    shipper_transport,
                    initial_connected,
                    block_size,
                    shipper_radio,
                    _shipper_packet_delay_ms,
                )
                .expect("Shipper creation failed");
                shipper.receive_msg_loop();
            });
        }
        loop {
            match self.transport.receive() {
                Ok((message, sender_addr)) => {
                    if self.addrs.insert(sender_addr.clone()) {
                        info!("Will sync to {sender_addr}");
                    }
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
                            error!("Error handling message (will send error response): {e}");
                            if let Err(e) =
                                self.transmit_response(Message::Error(e.to_string()), &sender_addr)
                            {
                                error!("TransmitResponse error: {e}");
                            }
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
                self.transmit_dag(&cid, &target_addr)?;
                shipper_sender.send((
                    DataProtocol::RequestTransmitDag {
                        cid: cid.clone(),
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
                let result = handlers::import_file(&path, &mut self.storage)?;
                match &result {
                    Message::ApplicationAPI(ApplicationAPI::FileImported { path, cid }) => {
                        if let Err(e) = self.upon_import(cid) {
                            error!("Error creating pushes corresponding to recent import of path {path:?}: {e:?}");
                        }
                    }
                    _ => error!(
                        "Unexpected and weird response to an import-file API request: {result:?} for path {path:?}"
                    ),
                }
                Some(result)
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
            Message::ApplicationAPI(ApplicationAPI::ListFiles) => {
                Some(handlers::get_named_dags(&self.storage)?)
            }
            Message::Sync(SyncMessage::Push(pm)) => {
                #[cfg(feature = "proto_sync")]
                {
                    self.sync_counts[1] += 1;
                    info!(
                        "Received a sync push, balance is now {:?}: {pm:?}",
                        self.sync_counts
                    );
                    self.sync.handle(SyncMessage::Push(pm), &mut self.storage)?
                }
                #[cfg(not(feature = "proto_sync"))]
                Some(Message::Error(format!(
                    "Sync protocol not implemented here. Received: {pm:?}"
                )))
            }
            Message::Sync(sm) => {
                #[cfg(feature = "proto_sync")]
                {
                    self.sync.handle(sm, &mut self.storage)?
                }
                #[cfg(not(feature = "proto_sync"))]
                {
                    Some(Message::Error(format!(
                        "Sync protocol not implemented here. Received: {sm:?}"
                    )))
                }
            }
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

    fn upon_import(&mut self, _root_cid_str: &str) -> Result<()> {
        #[cfg(feature = "proto_sync")]
        {
            let root = self.storage.get_block_by_cid(_root_cid_str)?;
            self.sync.push_dag(&root, false)?;
        }
        Ok(())
    }
    fn transmit_dag(&mut self, _root_cid_str: &str, _target: &str) -> Result<()> {
        #[cfg(feature = "proto_sync")]
        {
            let root = self.storage.get_block_by_cid(_root_cid_str)?;
            if let Some(immediate_msg) = self.sync.push_dag(&root, true)? {
                self.transmit_response(immediate_msg, _target)?;
            }
        }
        Ok(())
    }

    fn bg_tasks(&mut self) -> Result<()> {
        #[cfg(feature = "proto_sync")]
        if !self.addrs.is_empty() {
            trace!("Addrs to sync with: {:?}", &self.addrs);
            if let Some(msg) = self.sync.pop_pending_msg() {
                if matches!(&msg, Message::Sync(SyncMessage::Push(_))) {
                    self.sync_counts[0] += 1;
                }
                info!(
                    "Sending {msg:?} to {:?}, balance is now {:?}",
                    &self.addrs, self.sync_counts
                );
                //Sending a delayed Sync message, so bump that count
                for addr in &self.addrs {
                    self.transport.send(msg.clone(), addr)?;
                }
                return Ok(());
            }
        }
        if self.storage.incremental_gc() {
            debug!("GC run.");
        } else if self.sync_counts[0] > self.sync_counts[1] + self.sync_counts[2] {
            debug!(
                "Give the remote side a chance to talk. {:?}",
                &self.sync_counts
            );
            self.sync_counts[2] += 1;
        } else {
            self.sync_counts[2] = 0;
            #[cfg(feature = "proto_sync")]
            self.sync.build_msg()?;
        }
        Ok(())
    }
}

#[cfg(feature = "proto_sync")]
fn create_syncer(storage: &Storage, mtu: u16) -> Result<Syncer> {
    let missing_blocks = storage.get_provider().lock().unwrap().get_dangling_cids()?;
    let present_blocks = storage.list_available_dags()?;
    let present_blocks = present_blocks.iter().flat_map(|(c, n)| {
        let c = cid::Cid::try_from(c.as_str())?;
        Ok::<_, cid::Error>((c, n.clone()))
    });
    Ok(Syncer::new(mtu.into(), present_blocks, missing_blocks)?)
}
