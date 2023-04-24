use crate::handlers;
use anyhow::Result;
use cid::Cid;
use local_storage::block::StoredBlock;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::Message;
use messages::{DataProtocol, TransmissionBlock};
use std::collections::BTreeMap;
use std::net::ToSocketAddrs;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use tracing::{error, info};
use transports::Transport;

struct Session {
    pub remaining_retries: u8,
}

pub struct Shipper {
    // Handle to storage
    pub storage: Rc<Storage>,
    // Current shipping sessions
    sessions: BTreeMap<String, Session>,
    receiver: Receiver<(DataProtocol, String)>,
    sender: Sender<(DataProtocol, String)>,
    // Retry timeout in milliseconds
    retry_timeout_duration: u64,
    // Socket shared between listener and shipper for a consistent listening socket
    transport: Arc<dyn Transport + Send>,
}

impl Shipper {
    pub fn new(
        storage_path: &str,
        receiver: Receiver<(DataProtocol, String)>,
        sender: Sender<(DataProtocol, String)>,
        retry_timeout_duration: u64,
        transport: Arc<dyn Transport + Send>,
    ) -> Result<Shipper> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));

        Ok(Shipper {
            storage,
            sessions: BTreeMap::new(),
            receiver,
            sender,
            retry_timeout_duration,
            transport,
        })
    }

    pub fn receive_msg_loop(&mut self) {
        loop {
            if let Ok((message, sender_addr)) = self.receiver.recv() {
                if let Err(e) = self.receive(message, &sender_addr) {
                    error!("{e:?}");
                }
            }
        }
    }

    pub fn receive(&mut self, message: DataProtocol, sender_addr: &str) -> Result<()> {
        match message {
            DataProtocol::Block(block) => self.receive_block(block)?,
            DataProtocol::RequestMissingDagBlocks { cid } => {
                let missing_blocks_msg =
                    handlers::get_missing_dag_blocks_protocol(&cid, Rc::clone(&self.storage))?;
                self.transmit_msg(missing_blocks_msg, sender_addr)?;
            }
            DataProtocol::MissingDagBlocks { cid, blocks } => {
                if blocks.is_empty() {
                    info!("Dag transmission complete for {cid}");
                    self.sessions.remove(&cid);
                } else {
                    info!(
                        "Dag {cid} is missing {} blocks, sending again",
                        blocks.len()
                    );
                    let mut blocks_to_send = vec![];
                    for block in blocks {
                        blocks_to_send.push(self.storage.get_block_by_cid(&block)?);
                    }
                    self.transmit_blocks(&blocks_to_send, sender_addr)?;
                }
            }
            DataProtocol::RequestTransmitBlock { cid, target_addr } => {
                self.transmit_block(&cid, &target_addr)?;
            }
            DataProtocol::RequestTransmitDag {
                cid,
                target_addr,
                retries,
            } => {
                if retries == 0 {
                    self.transmit_dag(&cid, &target_addr)?;
                } else {
                    self.begin_dag_session(&cid, &target_addr, retries)?;
                }
            }
            DataProtocol::RetryDagSession { cid, target_addr } => {
                if self.sessions.contains_key(&cid) {
                    info!("Received retry dag session, sending get missing req to {target_addr}");
                    self.transmit_msg(Message::request_missing_dag_blocks(&cid), &target_addr)?;
                    self.retry_dag_session(&cid, &target_addr);
                }
            }
        }
        Ok(())
    }

    fn open_dag_session(&mut self, cid: &str, retries: u8) {
        self.sessions.entry(cid.to_owned()).or_insert(Session {
            remaining_retries: retries,
        });
    }

    fn start_retry_timeout(&mut self, cid: &str, target_addr: &str) {
        let sender_clone = self.sender.clone();
        let cid_str = cid.to_string();
        let target_addr_str = target_addr.to_string();
        let timeout_duration = Duration::from_millis(self.retry_timeout_duration);
        spawn(move || {
            sleep(timeout_duration);
            sender_clone
                .send((
                    DataProtocol::RetryDagSession {
                        cid: cid_str,
                        target_addr: target_addr_str,
                    },
                    "127.0.0.1:0".to_string(),
                ))
                .unwrap();
        });
    }

    fn retry_dag_session(&mut self, cid: &str, target_addr: &str) {
        if let Some(session) = self.sessions.get_mut(cid) {
            if session.remaining_retries > 0 {
                session.remaining_retries -= 1;
                self.start_retry_timeout(cid, target_addr);
            } else {
                self.sessions.remove(cid);
            }
        }
    }

    pub fn begin_dag_session(&mut self, cid: &str, target_addr: &str, retries: u8) -> Result<()> {
        // If we know we are beginning a dag transmission, then let's attempt to send the whole dag
        // at the start. afterwards we can ask about which pieces made it through and continue from there
        self.transmit_dag(cid, target_addr)?;
        self.transmit_msg(Message::request_missing_dag_blocks(cid), target_addr)?;
        self.open_dag_session(cid, retries - 1);
        self.start_retry_timeout(cid, target_addr);
        Ok(())
    }

    fn transmit_msg(&mut self, msg: Message, target_addr: &str) -> Result<()> {
        let resolved_target_addr = target_addr.to_socket_addrs().unwrap().next().unwrap();
        info!("sending {msg:?} to {resolved_target_addr}");
        // let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
        // let socket = UdpSocket::bind(bind_address)?;
        self.transport.send(msg, target_addr)?;
        Ok(())
    }

    pub fn transmit_blocks(&mut self, blocks: &[StoredBlock], target_addr: &str) -> Result<()> {
        info!("Transmitting {} blocks to {}", blocks.len(), target_addr);

        for block in blocks {
            let transmission = stored_block_to_transmission_block(block)?;

            info!(
                "Transmitting block {} to {target_addr}",
                block.cid.to_string()
            );
            self.transmit_msg(Message::data_block(transmission), target_addr)?;
        }

        Ok(())
    }

    pub fn transmit_block(&mut self, cid: &str, target_addr: &str) -> Result<()> {
        let block = self.storage.get_block_by_cid(cid)?;
        self.transmit_blocks(&[block], target_addr)?;
        Ok(())
    }

    pub fn transmit_dag(&mut self, cid: &str, target_addr: &str) -> Result<()> {
        let root_block = self.storage.get_block_by_cid(cid)?;
        let blocks = self.storage.get_all_blocks_under_cid(cid)?;
        let mut all_blocks = vec![root_block];
        all_blocks.extend(blocks);
        self.transmit_blocks(&all_blocks, target_addr)?;
        Ok(())
    }

    fn receive_block(&mut self, block: TransmissionBlock) -> Result<()> {
        let mut links = vec![];
        for l in block.links {
            links.push(Cid::try_from(l)?.to_string());
        }
        let stored_block = StoredBlock {
            cid: Cid::try_from(block.cid)?.to_string(),
            data: block.data,
            links,
        };
        stored_block.validate()?;
        self.storage.import_block(&stored_block)
    }
}

fn stored_block_to_transmission_block(stored: &StoredBlock) -> Result<TransmissionBlock> {
    let mut links = vec![];
    for l in stored.links.iter() {
        links.push(Cid::try_from(l.to_owned())?.to_bytes());
    }
    let block_cid = Cid::try_from(stored.cid.to_owned())?;

    Ok(TransmissionBlock {
        cid: block_cid.to_bytes(),
        data: stored.data.to_vec(),
        links,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_fs::fixture::FileWriteBin;
    use assert_fs::{fixture::PathChild, TempDir};
    use cid::multihash::MultihashDigest;
    use cid::Cid;
    use local_storage::provider::SqliteStorageProvider;
    use messages::{DataProtocol, Message, TransmissionBlock};
    use rand::{thread_rng, Rng, RngCore};
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;
    use transports::UdpTransport;

    struct TestShipper {
        listen_addr: String,
        listen_transport: Arc<dyn Transport + Send>,
        _storage: Rc<Storage>,
        shipper: Shipper,
        test_dir: TempDir,
    }

    impl TestShipper {
        pub fn new() -> Self {
            let mut rng = thread_rng();
            let port_num = rng.gen_range(6000..9000);
            let listen_addr = format!("127.0.0.1:{port_num}");
            let mut listen_transport = UdpTransport::new(&listen_addr, 60).unwrap();
            listen_transport
                .set_read_timeout(Some(Duration::from_millis(10)))
                .unwrap();
            listen_transport.set_max_read_attempts(Some(1));
            let listen_transport = Arc::new(listen_transport);
            let shipper_transport = Arc::clone(&listen_transport);

            let test_dir = TempDir::new().unwrap();
            let db_path = test_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            let _storage = Rc::new(Storage::new(Box::new(provider)));
            let (shipper_sender, shipper_receiver) = mpsc::channel();

            let shipper = Shipper::new(
                db_path.to_str().unwrap(),
                shipper_receiver,
                shipper_sender,
                10,
                shipper_transport,
            )
            .unwrap();
            TestShipper {
                listen_addr,
                _storage,
                shipper,
                test_dir,
                listen_transport,
            }
        }

        pub fn recv_msg(&mut self) -> Result<Message> {
            let (msg, _) = self.listen_transport.receive()?;
            Ok(msg)
        }

        pub fn generate_file(&self) -> Result<String> {
            let mut data = Vec::<u8>::new();
            data.resize(256, 1);
            thread_rng().fill_bytes(&mut data);

            let tmp_file = self.test_dir.child("test.file");
            tmp_file.write_binary(&data)?;
            Ok(tmp_file.path().to_str().unwrap().to_owned())
        }
    }

    #[test]
    pub fn test_receive_block_msg() {
        let mut harness = TestShipper::new();
        let data = b"1871217171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        let block_msg = DataProtocol::Block(TransmissionBlock {
            cid: cid.to_bytes(),
            data,
            links: vec![],
        });

        let res = harness.shipper.receive(block_msg, "127.0.0.1:8080");
        assert!(res.is_ok());

        let blocks = harness._storage.list_available_cids().unwrap();
        assert_eq!(blocks, vec![cid.to_string()]);
    }

    #[test]
    pub fn test_receive_block_msg_twice() {
        let mut harness = TestShipper::new();
        let data = b"18712552224417171".to_vec();
        let cid = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(&data));

        let block_msg = DataProtocol::Block(TransmissionBlock {
            cid: cid.to_bytes(),
            data,
            links: vec![],
        });

        let res = harness.shipper.receive(block_msg.clone(), "127.0.0.1:8080");
        assert!(res.is_ok());

        let res = harness.shipper.receive(block_msg, "127.0.0.1:8080");
        assert!(res.is_ok());

        let blocks = harness._storage.list_available_cids().unwrap();
        assert_eq!(blocks, vec![cid.to_string()]);
    }

    #[test]
    pub fn test_dag_transmit() {
        let mut transmitter = TestShipper::new();
        let mut receiver = TestShipper::new();

        // Generate file for test
        let test_file_path = transmitter.generate_file().unwrap();

        // Import test file into transmitter storage
        let test_file_cid = transmitter
            ._storage
            .import_path(&PathBuf::from(test_file_path))
            .unwrap();

        transmitter
            .shipper
            .receive(
                DataProtocol::RequestTransmitDag {
                    cid: test_file_cid.to_owned(),
                    target_addr: receiver.listen_addr.to_owned(),
                    retries: 0,
                },
                "127.0.0.1:0",
            )
            .unwrap();

        // receive pump
        while let Ok(Message::DataProtocol(msg)) = receiver.recv_msg() {
            receiver
                .shipper
                .receive(msg, &transmitter.listen_addr)
                .unwrap();
        }

        // Verify all blocks made it across
        receiver
            .shipper
            .receive(
                DataProtocol::RequestMissingDagBlocks {
                    cid: test_file_cid.to_owned(),
                },
                &transmitter.listen_addr,
            )
            .unwrap();
        let missing_blocks_msg = transmitter.recv_msg().unwrap();
        assert_eq!(
            missing_blocks_msg,
            Message::missing_dag_blocks(&test_file_cid, vec![])
        );
    }
}
