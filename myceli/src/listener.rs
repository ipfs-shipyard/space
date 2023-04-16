use crate::handlers;
use crate::shipper::Shipper;
use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::{
    ApplicationAPI, DagInfo, DataProtocol, Message, MessageChunker, SimpleChunker, UnchunkResult,
};
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
use tracing::{error, info, warn};

pub struct Listener {
    storage_path: String,
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    chunker: Arc<Mutex<SimpleChunker>>,
    socket: Arc<UdpSocket>,
    mtu: u16,
    node_name: String,
    nodes_list: Arc<Mutex<BTreeMap<String, Option<String>>>>,
    radio_address: SocketAddr,
    network_dags: Arc<Mutex<BTreeMap<String, DagInfo>>>,
    primary: bool,
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
        primary: bool,
    ) -> Result<Self> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        info!("{node_name} listening on {listen_address}");
        let socket = UdpSocket::bind(listen_address)?;
        let _ = socket.set_read_timeout(Some(Duration::from_millis(100)));
        let radio_address = radio_address
            .to_socket_addrs()
            .map(|mut i| i.next().unwrap())
            .unwrap();
        Ok(Listener {
            storage_path: storage_path.to_string(),
            storage,
            sender_addr: None,
            chunker: Arc::new(Mutex::new(SimpleChunker::new(mtu))),
            socket: Arc::new(socket),
            mtu,
            node_name: node_name.to_string(),
            nodes_list,
            radio_address,
            network_dags,
            primary,
        })
    }

    pub fn start(&mut self, shipper_timeout_duration: u64) -> Result<()> {
        // First setup the shipper and its pieces
        let (shipper_sender, shipper_receiver) = mpsc::channel();
        let shipper_storage_path = self.storage_path.to_string();
        let shipper_sender_clone = shipper_sender.clone();
        let shipper_socket = Arc::clone(&self.socket);
        let shipper_mtu = self.mtu;
        let shipper_chunker = Arc::clone(&self.chunker);
        spawn(move || {
            let mut shipper = Shipper::new(
                &shipper_storage_path,
                shipper_receiver,
                shipper_sender_clone,
                shipper_timeout_duration,
                shipper_socket,
                shipper_chunker,
                shipper_mtu,
            )
            .expect("Shipper creation failed");
            shipper.receive_msg_loop();
        });

        if self.primary {
            self.network_ping()?;
        }

        let mut buf = vec![0; usize::from(self.mtu)];
        let mut receive_tries = 0;
        loop {
            {
                loop {
                    if receive_tries > 100 {
                        // info!("50 recv tries, check for missing chunks");
                        receive_tries = 0;
                        let missing_chunks = self.chunker.lock().unwrap().find_missing_chunks()?;
                        if !missing_chunks.is_empty() {
                            // println!("Found {} missing chunk msgs", missing_chunks.len());
                            for msg in missing_chunks {
                                self.socket.send_to(&msg, self.sender_addr.unwrap())?;
                            }
                        } else {
                            // info!("nothing missing!");
                        }
                    }

                    receive_tries += 1;
                    match self.socket.recv_from(&mut buf) {
                        Ok((len, sender)) => {
                            if len > 0 {
                                self.sender_addr = Some(sender);
                                receive_tries -= 1;
                                break;
                            }
                        }
                        Err(e) => match e.kind() {
                            std::io::ErrorKind::WouldBlock => {}
                            other => error!("Recv failed: {other}"),
                        },
                    }
                    sleep(Duration::from_millis(10));
                }
            }

            let unchunked_resp = self.chunker.lock().unwrap().unchunk(&buf);

            match unchunked_resp {
                Ok(Some(UnchunkResult::Message(msg))) => {
                    // info!("unchunked msg");
                    match self.handle_message(msg, shipper_sender.clone()) {
                        Ok(Some(resp)) => {
                            info!("Transmit resp {resp:?}");
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
                Ok(Some(UnchunkResult::Missing(missing))) => {
                    // info!("unchunked missing msg");
                    let missing_chunks = self
                        .chunker
                        .lock()
                        .unwrap()
                        .get_prev_sent_chunks(missing.0)?;
                    // println!("got missing chunks {}", missing_chunks.len());
                    for chunk in missing_chunks {
                        self.socket.send_to(&chunk, self.sender_addr.unwrap())?;
                    }
                }
                Ok(None) => {
                    // info!("unchunked none");
                }
                Err(err) => {
                    error!("Message parse failed: {err}");
                }
            }

            // info!("general check for missing chunks");
            // receive_tries = 0;
            // let missing_chunks = self.chunker.lock().unwrap().find_missing_chunks()?;
            // if !missing_chunks.len() > 1 {
            //     println!("found we are missing {} chunks", missing_chunks.len());

            //     self.socket
            //         .send_to(&missing_chunks, self.sender_addr.unwrap())?;
            // } else {
            //     info!("nothing missing!");
            // }
        }
    }

    fn handle_message(
        &mut self,
        message: Message,
        shipper_sender: Sender<(DataProtocol, String)>,
    ) -> Result<Option<Message>> {
        info!("Handling msg {message:?}");
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
                            _other => {}
                        }
                        None
                    }
                }
            }
            Message::ApplicationAPI(ApplicationAPI::FileImported { path, cid }) => {
                if let Some(info) = self.nodes_list.lock().unwrap().last_entry() {
                    if let Some(filename) = PathBuf::from(path).file_name().and_then(|s| s.to_str())
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
                info!("req available dags");
                info!("check local dags");
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
                info!("check network dags");
                let mut network_dags: Vec<DagInfo> = self
                    .network_dags
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(_cid, info)| info.clone())
                    .collect();
                local_dags.append(&mut network_dags);
                dbg!(&local_dags);
                info!("return");
                Some(Message::ApplicationAPI(ApplicationAPI::AvailableDags {
                    dags: local_dags,
                }))
                // Ok(Message::ApplicationAPI(ApplicationAPI::AvailableDags {
                //     dags,
                // }))
                // Some(handlers::request_available_dags(self.storage.clone())?)
            }
            Message::ApplicationAPI(ApplicationAPI::AvailableDags { dags }) => {
                info!("Found dags from networked myceli");
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
                            target_addr: "127.0.0.1:8081".to_string(),
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
                // And then return our own
                Some(Message::ApplicationAPI(ApplicationAPI::NodeAddress {
                    address: self.node_name.to_string(),
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::NodeAddress { address }) => {
                // For today we'll assume that if we receive a ::NodeAddress message that it came from the other myceli
                info!("Found myceli node: {address}");
                self.nodes_list
                    .lock()
                    .unwrap()
                    .insert(address, Some(self.radio_address.to_string()));
                None
            }
            Message::ApplicationAPI(ApplicationAPI::RequestNodeList) => {
                // Request radio's node addr if we don't have it already
                // if self.nodes_list.lock().unwrap().len() != 2 {
                //     self.transmit_msg(
                //         Message::ApplicationAPI(ApplicationAPI::RequestNodeAddress),
                //         self.radio_address,
                //     )?;
                // }

                Some(Message::ApplicationAPI(ApplicationAPI::NodeList {
                    nodes: self.nodes_list.lock().unwrap().clone(),
                }))
            }
            Message::ApplicationAPI(ApplicationAPI::NetworkPing) => {
                self.transmit_msg(
                    Message::ApplicationAPI(ApplicationAPI::NodeAddress {
                        address: self.node_name.to_string(),
                    }),
                    self.sender_addr.unwrap(),
                )?;

                let local_dags: Vec<DagInfo> = self
                    .storage
                    .list_available_dags()?
                    .iter()
                    .map(|(cid, filename)| DagInfo {
                        cid: cid.to_string(),
                        filename: filename.to_string(),
                        node: self.node_name.to_string(),
                    })
                    .collect();
                self.transmit_msg(
                    Message::ApplicationAPI(ApplicationAPI::AvailableDags { dags: local_dags }),
                    self.sender_addr.unwrap(),
                )?;
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

    fn transmit_msg_str_addr(&self, message: Message, target_addr: &str) -> Result<()> {
        let target_addr = target_addr
            .to_socket_addrs()
            .map(|mut s| s.next().unwrap())
            .unwrap();
        self.transmit_msg(message, target_addr)
    }

    fn transmit_msg(&self, message: Message, target_addr: SocketAddr) -> Result<()> {
        info!("transmit msg {message:?}");
        for chunk in self.chunker.lock().unwrap().chunk(message)? {
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

    fn network_ping(&self) -> Result<()> {
        info!("Pinging myceli network");
        self.transmit_msg(
            Message::ApplicationAPI(ApplicationAPI::NetworkPing),
            self.radio_address,
        )?;

        self.transmit_msg(
            Message::ApplicationAPI(ApplicationAPI::NodeAddress {
                address: self.node_name.to_string(),
            }),
            self.radio_address,
        )?;

        let local_dags: Vec<DagInfo> = self
            .storage
            .list_available_dags()?
            .iter()
            .map(|(cid, filename)| DagInfo {
                cid: cid.to_string(),
                filename: filename.to_string(),
                node: self.node_name.to_string(),
            })
            .collect();
        self.transmit_msg(
            Message::ApplicationAPI(ApplicationAPI::AvailableDags { dags: local_dags }),
            self.radio_address,
        )?;

        Ok(())
    }
}
