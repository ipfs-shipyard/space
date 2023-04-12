use crate::handlers;
use crate::shipper::Shipper;
use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::{ApplicationAPI, DagInfo, DataProtocol, Message, MessageChunker, SimpleChunker};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct Listener {
    storage_path: String,
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    chunker: SimpleChunker,
    socket: Arc<UdpSocket>,
    mtu: u16,
    node_name: String,
    nodes_list: Arc<Mutex<BTreeMap<String, Option<String>>>>,
    radio_address: SocketAddr,
    network_dags: Arc<Mutex<BTreeMap<String, DagInfo>>>,
}

impl Listener {
    pub fn new(
        listen_address: &SocketAddr,
        storage_path: &str,
        mtu: u16,
        node_name: &str,
        nodes_list: Arc<Mutex<BTreeMap<String, Option<String>>>>,
        radio_address: &str,
        network_dags: Arc<Mutex<BTreeMap<String, DagInfo>>>,
    ) -> Result<Self> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        info!("{node_name} listening on {listen_address}");
        let socket = UdpSocket::bind(listen_address)?;
        let radio_address = radio_address
            .to_socket_addrs()
            .map(|mut i| i.next().unwrap())
            .unwrap();
        Ok(Listener {
            storage_path: storage_path.to_string(),
            storage,
            sender_addr: None,
            chunker: SimpleChunker::new(mtu),
            socket: Arc::new(socket),
            mtu,
            node_name: node_name.to_string(),
            nodes_list,
            radio_address,
            network_dags,
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

            println!("datadbg: {buf:?}");

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
                    info!("No msg found yet");
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
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path, target_node }) => {
                match target_node {
                    None => {
                        info!("got import file for local fine, handling here");
                        Some(handlers::import_file(&path, self.storage.clone())?)
                    }
                    Some(target_node) => {
                        info!("got import file for node {target_node}, checking nodes list");
                        match self.nodes_list.lock().unwrap().get(&target_node) {
                            Some(Some(target_addr)) => {
                                info!("Node entry with {target_addr}, sending away");
                                self.transmit_msg_str_addr(
                                    Message::import_file(&path, None),
                                    target_addr,
                                )?;
                            }
                            other => {}
                        }
                        None
                    }
                }
            }
            Message::ApplicationAPI(ApplicationAPI::FileImported { path, cid }) => {
                if let Some(info) = self.nodes_list.lock().unwrap().last_entry() {
                    if let Some(filename) = PathBuf::from(path)
                        .file_name()
                        .map(|s| s.to_str())
                        .flatten()
                    {
                        self.network_dags.lock().unwrap().insert(
                            cid.to_string(),
                            DagInfo {
                                cid,
                                filename: filename.to_string(),
                                node: info.key().to_string(),
                            },
                        );
                    }
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path }) => {
                self.storage.export_cid(&cid, &PathBuf::from(path))?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestAvailableBlocks) => {
                Some(handlers::request_available_blocks(self.storage.clone())?)
            }
            Message::ApplicationAPI(ApplicationAPI::RequestAvailableDags) => {
                let mut local_dags: Vec<DagInfo> = self
                    .storage
                    .list_available_dags()?
                    .iter()
                    .map(|(cid, filename)| DagInfo {
                        cid: cid.to_string(),
                        filename: filename.to_string(),
                        node: self.node_name.to_string(),
                    })
                    .collect();
                let mut network_dags: Vec<DagInfo> = self
                    .network_dags
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(cid, info)| info.clone())
                    .collect();
                local_dags.append(&mut network_dags);
                Some(Message::ApplicationAPI(ApplicationAPI::AvailableDags {
                    dags: local_dags,
                }))
                // Ok(Message::ApplicationAPI(ApplicationAPI::AvailableDags {
                //     dags,
                // }))
                // Some(handlers::request_available_dags(self.storage.clone())?)
            }
            Message::ApplicationAPI(ApplicationAPI::AvailableDags { dags }) => {
                for info in dags {
                    self.network_dags
                        .lock()
                        .unwrap()
                        .insert(info.cid.to_string(), info.clone());
                }
                None
            }
            Message::ApplicationAPI(ApplicationAPI::GetMissingDagBlocks { cid }) => Some(
                handlers::get_missing_dag_blocks(&cid, self.storage.clone())?,
            ),
            Message::ApplicationAPI(ApplicationAPI::ValidateDag { cid }) => {
                Some(handlers::validate_dag(&cid, self.storage.clone())?)
            }
            Message::ApplicationAPI(ApplicationAPI::FetchDag { cid }) => {
                if let Some(info) = self.network_dags.lock().unwrap().get(&cid) {
                    info!("Found dag for {} at node {}", info.filename, info.node);
                    self.transmit_msg(
                        Message::ApplicationAPI(ApplicationAPI::TransmitDag {
                            cid,
                            target_addr: "127.0.0.1:8080".to_string(),
                            retries: 5,
                        }),
                        self.radio_address,
                    )?;
                } else {
                    warn!("Dag for {cid} not found in network");
                }

                None
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
            Message::ApplicationAPI(ApplicationAPI::RequestNodeAddress) => {
                // Request radio's node addr if we don't have it already
                if self.nodes_list.lock().unwrap().len() != 2 {
                    self.transmit_msg(
                        Message::ApplicationAPI(ApplicationAPI::RequestNodeAddress),
                        self.radio_address,
                    )?;
                }
                // And then return our own
                Some(Message::ApplicationAPI(ApplicationAPI::NodeAddress {
                    address: self.node_name.to_string(),
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::NodeAddress { address }) => {
                // For today we'll assume that if we receive a ::NodeAddress message that it came from the other myceli
                self.nodes_list
                    .lock()
                    .unwrap()
                    .insert(address, Some(self.radio_address.to_string()));
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestNodeList) => {
                Some(Message::ApplicationAPI(ApplicationAPI::NodeList {
                    nodes: self.nodes_list.lock().unwrap().clone(),
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

    fn transmit_msg_str_addr(&self, message: Message, target_addr: &str) -> Result<()> {
        let target_addr = target_addr
            .to_socket_addrs()
            .map(|mut s| s.next().unwrap())
            .unwrap();
        self.transmit_msg(message, target_addr)
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
