use crate::handlers;
#[cfg(feature = "proto_ship")]
use crate::shipper::Shipper;
#[cfg(feature = "proto_sync")]
use crate::sync::Syncer;
use anyhow::Result;
use local_storage::{provider::default_storage_provider, storage::Storage};
use log::{debug, error, info, trace, warn};
#[cfg(feature = "proto_ship")]
use messages::DataProtocol;
use messages::{ApplicationAPI, Message, SyncMessage};
use std::collections::BTreeSet;
#[cfg(feature = "proto_ship")]
use std::{
    iter,
    sync::mpsc::{self, Sender},
    thread::spawn,
};
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use transports::{Transport, TransportError};

pub struct Listener<T> {
    storage: Storage,
    transport: Arc<T>,
    connected: Arc<Mutex<bool>>,
    radio_address: Option<String>,
    ship_target_addrs: BTreeSet<String>,
    sync_target_addrs: BTreeSet<String>,
    _block_size: u32,
    #[cfg(feature = "proto_sync")]
    sync_counts: [u64; 3],
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
        #[cfg(feature = "proto_sync")]
        let sync = create_syncer(&storage, _mtu)?;
        Ok(Listener {
            storage,
            transport,
            connected: Arc::new(Mutex::new(true)),
            radio_address,
            sync_target_addrs: BTreeSet::default(),
            ship_target_addrs: BTreeSet::default(),
            _block_size: block_size,
            #[cfg(feature = "proto_sync")]
            sync_counts: [0; 3],
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
        #[cfg(feature = "proto_ship")]
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
                    if matches!(&message, Message::Sync(_))
                        && self.sync_target_addrs.insert(sender_addr.clone())
                    {
                        info!("Will sync to {sender_addr}");
                    }
                    if let Some(target_addr) = message.target_addr() {
                        if self.ship_target_addrs.insert(target_addr.clone()) {
                            info!("Will send to {target_addr} with shipper.");
                        }
                    }
                    match {
                        #[cfg(feature = "proto_ship")]
                        {
                            self.handle_message(
                                message,
                                &sender_addr.to_owned(),
                                shipper_sender.clone(),
                            )
                        }
                        #[cfg(not(feature = "proto_ship"))]
                        self.handle_message(message, &sender_addr)
                    } {
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
        sender: &str,
        #[cfg(feature = "proto_ship")] shipper_sender: Sender<(DataProtocol, String)>,
    ) -> Result<Option<Message>> {
        trace!("Handling {message:?} from {sender}");
        #[cfg(feature = "proto_ship")]
        let ship = |this: &Self, m: DataProtocol| {
            for t in this
                .ship_target_addrs
                .iter()
                .map(|s| s.as_str())
                .chain(iter::once(sender))
            {
                if let Err(e) = shipper_sender.send((m.clone(), t.to_string())) {
                    error!("Error sending message to shipper for {t}: {e:?} msg={m:?}");
                }
            }
        };
        let resp: Option<Message> = match message {
            Message::ApplicationAPI(ApplicationAPI::TransmitDag {
                cid,
                target_addr,
                retries: _retries,
            }) => {
                self.transmit_dag(&cid, &target_addr)?;
                #[cfg(feature = "proto_ship")]
                ship(
                    self,
                    DataProtocol::RequestTransmitDag {
                        cid: cid.clone(),
                        target_addr,
                        retries: _retries,
                    },
                );
                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitBlock { cid, target_addr }) => {
                self.transmit_dag(&cid, &target_addr)?;
                #[cfg(feature = "proto_ship")]
                ship(
                    self,
                    DataProtocol::RequestTransmitBlock { cid, target_addr },
                );
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
            Message::DataProtocol(_data_msg) => {
                self.ship_target_addrs.insert(sender.to_string());
                #[cfg(feature = "proto_ship")]
                ship(self, _data_msg);
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestVersion { label }) => {
                info!("Remote {sender} requested version info labelled with {label:?}");
                Some(Message::ApplicationAPI(crate::version_info::get(label)))
            }
            Message::ApplicationAPI(ApplicationAPI::Version {
                version,
                features,
                profile,
                target,
                rust,
                remote_label,
            }) => {
                info!("Remote {sender} aka {remote_label:?}: myceli {version} built by rust {rust} for {target} on profile {profile} using these features: {features:?}");
                let their_version = version;
                if let ApplicationAPI::Version { version, .. } = crate::version_info::get(None) {
                    let my_version = version;
                    if let Some(mismatch_component) = my_version
                        .split('.')
                        .zip(their_version.split('.'))
                        .position(|(a, b)| a != b)
                    {
                        if mismatch_component < 2 {
                            if let Some(radio) = &self.radio_address {
                                if sender == radio {
                                    panic!("Versions are TOO different, can't expect backward compatibility that far. mine={my_version} theirs(configured as radio_address={sender})={their_version}");
                                }
                            }
                            error!("Versions are TOO different, can't expect backward compatibility that far. mine={my_version} theirs({sender})={their_version}");
                        }
                    }
                    let _remote = remote_label.unwrap_or(sender.to_owned());
                    #[cfg(feature = "proto_sync")]
                    if features.iter().any(|f| f == "PROTO_SYNC")
                        && self.sync_target_addrs.insert(_remote.clone())
                    {
                        info!("Remote {_remote} reported that it supports sync protocol, so adding it to addresses to target with that.");
                    }
                    #[cfg(feature = "proto_ship")]
                    if features.iter().any(|f| f == "PROTO_SHIP")
                        && self.ship_target_addrs.insert(_remote.clone())
                    {
                        info!("Remote {_remote} reported that it supports ship protocol, so adding it to addresses to target with that.");
                    }
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::SetConnected { connected }) => {
                let prev_connected = *self.connected.lock().unwrap();
                *self.connected.lock().unwrap() = connected;
                if !prev_connected && connected {
                    #[cfg(feature = "proto_ship")]
                    ship(self, DataProtocol::ResumeTransmitAllDags);
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::GetConnected) => {
                Some(Message::ApplicationAPI(ApplicationAPI::ConnectedState {
                    connected: *self.connected.lock().unwrap(),
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::ResumeTransmitDag { cid }) => {
                self.transmit_dag(&cid, sender)?;
                #[cfg(feature = "proto_ship")]
                ship(self, DataProtocol::ResumeTransmitDag { cid });
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ResumeTransmitAllDags) => {
                #[cfg(feature = "proto_ship")]
                ship(self, DataProtocol::ResumeTransmitAllDags);
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse { cid, result }) => {
                info!("Received ValidateDagResponse from {sender} for {cid}: {result}");
                None
            }
            Message::ApplicationAPI(ApplicationAPI::FileImported { path, cid }) => {
                info!("Received FileImported from {sender}: {path} -> {cid}");
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
            Message::Error(err_msg) => {
                error!("Received an error message from remote: {err_msg}");
                None
            }
            Message::ApplicationAPI(api_msg) => {
                //Catch-all for API messages that have no handling code - typically responses
                warn!("Received unsupported API message: {api_msg:?}");
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
        if let Some(radio) = &self.radio_address {
            if self.sync_target_addrs.contains(radio) {
                trace!("Configured radio {radio} is a sync target");
            } else if self.ship_target_addrs.contains(radio) {
                trace!("Configured radio {radio} is a ship target");
            } else {
                debug!("Requesting version info & supported protocols from '{radio}' since it doesn't appear in ship {:?} OR sync {:?}", &self.ship_target_addrs, &self.sync_target_addrs);
                self.transport.send(
                    Message::ApplicationAPI(crate::version_info::get(None)),
                    radio,
                )?;
                self.transport
                    .send(Message::request_version(radio.clone()), radio)?;
                return Ok(());
            }
        } else {
            trace!("No configured radio address - will assume they'll contact me to let me know protocols.");
        }
        trace!("sync_target_addrs={:?}", self.sync_target_addrs);
        #[cfg(feature = "proto_sync")]
        if !self.sync_target_addrs.is_empty() {
            trace!("Addrs to sync with: {:?}", &self.sync_target_addrs);
            if let Some(msg) = self.sync.pop_pending_msg(&self.storage) {
                if matches!(&msg, Message::Sync(SyncMessage::Push(_))) {
                    self.sync_counts[0] += 1;
                }
                info!(
                    "Sending {msg:?} to {:?}, balance is now {:?}",
                    &self.sync_target_addrs, self.sync_counts
                );
                //Sending a delayed Sync message, so bump that count
                for addr in &self.sync_target_addrs {
                    self.transport.send(msg.clone(), addr)?;
                }
                return Ok(());
            } else {
                trace!("No already-pending Sync messages ready to send.");
            }
        }
        if self.storage.incremental_gc() {
            debug!("GC run.");
            return Ok(());
        }
        #[cfg(feature = "proto_sync")]
        if self.sync_counts[0] > self.sync_counts[1] + self.sync_counts[2] {
            debug!(
                "Give the remote side a chance to talk. {:?}",
                &self.sync_counts
            );
            self.sync_counts[2] += 1;
        } else {
            trace!("Will try to build a new Sync message");
            self.sync_counts[2] = 0;

            if let Err(e) = self.sync.build_msg(&mut self.storage) {
                error!("Error while building a new Sync message to send: {e:?}");
            }
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
    Syncer::new(mtu.into(), present_blocks, missing_blocks)
}
