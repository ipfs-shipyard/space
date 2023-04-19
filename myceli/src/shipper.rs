use crate::handlers;
use anyhow::Result;
use cid::Cid;
use local_storage::block::StoredBlock;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use messages::Message;
use messages::{ApplicationAPI, DataProtocol, TransmissionBlock};
use messages::{MessageChunker, SimpleChunker};
use std::collections::BTreeMap;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::{sleep, spawn};
use std::time::Duration;
use tracing::{error, info};

struct Session {
    pub remaining_retries: u8,
}

struct WindowSession {
    pub remaining_retries: u8,
    pub window_num: u8,
    pub window_size: u8,
}

pub struct Shipper {
    // Handle to storage
    pub storage: Rc<Storage>,
    // Current shipping sessions
    sessions: BTreeMap<String, Session>,
    window_sessions: BTreeMap<String, WindowSession>,
    receiver: Receiver<(DataProtocol, String)>,
    sender: Sender<(DataProtocol, String)>,
    // Retry timeout in milliseconds
    retry_timeout_duration: u64,
    // Socket shared between listener and shipper for a consistent listening socket
    socket: Arc<UdpSocket>,
    chunker: Arc<Mutex<SimpleChunker>>,
    mtu: u16,
}

impl Shipper {
    pub fn new(
        storage_path: &str,
        receiver: Receiver<(DataProtocol, String)>,
        sender: Sender<(DataProtocol, String)>,
        retry_timeout_duration: u64,
        socket: Arc<UdpSocket>,
        chunker: Arc<Mutex<SimpleChunker>>,
        mtu: u16,
    ) -> Result<Shipper> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));

        Ok(Shipper {
            storage,
            sessions: BTreeMap::new(),
            window_sessions: BTreeMap::new(),
            receiver,
            sender,
            retry_timeout_duration,
            socket,
            chunker,
            mtu,
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
            DataProtocol::RetryDagWindowSession { cid, target_addr } => {
                if self.window_sessions.contains_key(&cid) {
                    info!("Received retry dag session, sending get missing req to {target_addr}");
                    if let Some(session) = self.window_sessions.get(&cid) {
                        let blocks = self.get_dag_window_blocks(
                            &cid,
                            session.window_num,
                            session.window_size,
                        )?;
                        let blocks = blocks.iter().map(|s| s.cid.to_string()).collect();
                        self.transmit_msg(
                            Message::DataProtocol(DataProtocol::RequestMissingDagBlocksWindow {
                                cid: cid.to_string(),
                                blocks,
                            }),
                            &target_addr,
                        )?;
                        self.retry_dag_window_session(&cid, &target_addr);
                    }
                }
            }
            DataProtocol::RequestTransmitDagWindow {
                cid,
                target_addr,
                retries,
                window_size,
            } => {
                self.begin_dag_window_session(&cid, &target_addr, retries, window_size)?;
            }
            DataProtocol::RequestMissingDagBlocksWindow { cid, blocks } => {
                let missing_blocks_msg = handlers::get_missing_dag_blocks_window_protocol(
                    &cid,
                    blocks,
                    Rc::clone(&self.storage),
                )?;
                self.transmit_msg(missing_blocks_msg, sender_addr)?;
            }
            DataProtocol::MissingDagBlocksWindow { cid, blocks } => {
                // If no blocks are missing, then attempt to move to next window
                if blocks.is_empty() {
                    self.increment_dag_window_session(&cid, sender_addr)?;
                } else {
                    for b in blocks.clone() {
                        self.transmit_block(&b, sender_addr)?;
                    }
                    self.transmit_msg(
                        Message::DataProtocol(DataProtocol::RequestMissingDagBlocksWindow {
                            cid,
                            blocks,
                        }),
                        sender_addr,
                    )?;
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

    fn open_dag_window_session(&mut self, cid: &str, retries: u8, window_size: u8) {
        self.window_sessions
            .entry(cid.to_string())
            .or_insert(WindowSession {
                remaining_retries: retries,
                window_num: 0,
                window_size,
            });
    }

    fn next_dag_window_session(&mut self, cid: &str) -> Option<(u8, u8)> {
        if let Some(session) = self.window_sessions.get_mut(cid) {
            session.window_num += 1;
            session.remaining_retries = 5;
            return Some((session.window_num, session.window_size));
        }
        None
    }

    fn end_dag_window_session(&mut self, cid: &str) {
        self.window_sessions.remove(cid);
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

    fn start_dag_window_retry_timeout(&mut self, cid: &str, target_addr: &str) {
        let sender_clone = self.sender.clone();
        let cid_str = cid.to_string();
        let target_addr_str = target_addr.to_string();
        let timeout_duration = Duration::from_millis(self.retry_timeout_duration);
        spawn(move || {
            sleep(timeout_duration);
            sender_clone
                .send((
                    DataProtocol::RetryDagWindowSession {
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

    fn retry_dag_window_session(&mut self, cid: &str, target_addr: &str) {
        if let Some(session) = self.window_sessions.get_mut(cid) {
            if session.remaining_retries > 0 {
                session.remaining_retries -= 1;
                self.start_dag_window_retry_timeout(cid, target_addr);
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

    pub fn begin_dag_window_session(
        &mut self,
        cid: &str,
        target_addr: &str,
        retries: u8,
        window_size: u8,
    ) -> Result<()> {
        let blocks = self.transmit_dag_window(cid, window_size, 0, target_addr)?;
        self.transmit_msg(
            Message::DataProtocol(DataProtocol::RequestMissingDagBlocksWindow {
                cid: cid.to_string(),
                blocks,
            }),
            target_addr,
        )?;
        self.open_dag_window_session(cid, retries - 1, window_size);
        self.start_dag_window_retry_timeout(cid, target_addr);

        Ok(())
    }

    pub fn increment_dag_window_session(&mut self, cid: &str, target_addr: &str) -> Result<()> {
        if let Some((next_window_num, window_size)) = self.next_dag_window_session(cid) {
            let blocks =
                self.transmit_dag_window(cid, window_size, next_window_num, target_addr)?;
            if !blocks.is_empty() {
                info!(
                    "Dag window session for {cid} moving to window {}",
                    next_window_num + 1
                );
                self.transmit_msg(
                    Message::DataProtocol(DataProtocol::RequestMissingDagBlocksWindow {
                        cid: cid.to_string(),
                        blocks,
                    }),
                    target_addr,
                )?;
            } else {
                info!("dag window session for {cid} is complete");
                self.end_dag_window_session(cid);
            }
        }
        Ok(())
    }

    fn transmit_msg(&mut self, msg: Message, target_addr: &str) -> Result<()> {
        // println!("resolving {target_addr}");
        let resolved_target_addr = target_addr.to_socket_addrs().unwrap().next().unwrap();
        info!("Transmitting {msg:?} to {resolved_target_addr}");
        // let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
        // let socket = UdpSocket::bind(bind_address)?;
        for chunk in self.chunker.lock().unwrap().chunk(msg)? {
            // info!("sending chunk {} away", chunk.len());
            self.socket.send_to(&chunk, resolved_target_addr)?;
        }
        // info!("done transmit");
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
        info!("found {} blocks under {} to transmit", blocks.len(), cid);
        let mut all_blocks = vec![root_block];
        all_blocks.extend(blocks);
        self.transmit_blocks(&all_blocks, target_addr)?;
        Ok(())
    }

    pub fn transmit_dag_window(
        &mut self,
        cid: &str,
        window_size: u8,
        window_num: u8,
        target_addr: &str,
    ) -> Result<Vec<String>> {
        let mut transmitted_cids = vec![];

        let window_blocks = self.get_dag_window_blocks(cid, window_num, window_size)?;

        info!(
            "transmitting {} blocks in window {}",
            window_blocks.len(),
            window_num,
        );

        self.transmit_blocks(&window_blocks, target_addr)?;
        for b in window_blocks {
            transmitted_cids.push(b.cid);
        }

        Ok(transmitted_cids)
    }

    fn get_dag_window_blocks(
        &mut self,
        cid: &str,
        window_num: u8,
        window_size: u8,
    ) -> Result<Vec<StoredBlock>> {
        let root_block = self.storage.get_block_by_cid(cid)?;
        let blocks = self.storage.get_all_blocks_under_cid(cid)?;
        let mut all_blocks = vec![root_block];
        all_blocks.extend(blocks);

        if let Some(window_blocks) = all_blocks
            .chunks(window_size.into())
            .map(|c| c.to_vec())
            .nth(window_num.into())
        {
            Ok(window_blocks)
        } else {
            Ok(vec![])
        }
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

    use anyhow::bail;
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

    struct TestShipper {
        listen_addr: String,
        listen_socket: Arc<UdpSocket>,
        _storage: Rc<Storage>,
        shipper: Shipper,
        test_dir: TempDir,
    }

    impl TestShipper {
        pub fn new() -> Self {
            let mut rng = thread_rng();
            let port_num = rng.gen_range(6000..9000);
            let listen_addr = format!("127.0.0.1:{port_num}");
            let listen_socket = Arc::new(UdpSocket::bind(listen_addr.to_owned()).unwrap());
            listen_socket
                .set_read_timeout(Some(Duration::from_millis(10)))
                .unwrap();
            let shipper_socket = Arc::clone(&listen_socket);

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
                shipper_socket,
                60,
            )
            .unwrap();
            TestShipper {
                listen_addr,
                _storage,
                shipper,
                test_dir,
                listen_socket,
            }
        }

        pub fn recv_msg(&mut self) -> Result<Message> {
            let mut tries = 0;
            let mut chunker = SimpleChunker::new(60);
            loop {
                let mut buf = vec![0; 128];
                if self.listen_socket.recv(&mut buf).is_ok() {
                    match chunker.unchunk(&buf) {
                        Ok(Some(msg)) => return Ok(msg),
                        Ok(None) => {}
                        Err(e) => bail!("Error found {e:?}"),
                    }
                }
                sleep(Duration::from_millis(10));
                tries += 1;
                if tries > 20 {
                    bail!("Listen tries exceeded");
                }
            }
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
