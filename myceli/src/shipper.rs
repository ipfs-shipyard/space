use crate::handlers;
use anyhow::{anyhow, Result};
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
use std::sync::Mutex;
use std::thread::{sleep, spawn};
use std::time::Duration;
use tracing::{error, info};
use transports::Transport;

#[derive(Clone)]
struct WindowSession {
    pub max_retries: u8,
    pub remaining_window_retries: u8,
    pub window_num: u32,
    pub target_addr: String,
}

pub struct Shipper<T> {
    // Handle to storage
    pub storage: Rc<Storage>,
    // Current windowed shipping sessions
    window_sessions: BTreeMap<String, WindowSession>,
    // Channel for receiving messages from Listener
    receiver: Receiver<(DataProtocol, String)>,
    // Channel for sending messages back to self
    sender: Sender<(DataProtocol, String)>,
    // Retry timeout in milliseconds
    retry_timeout_duration: u64,
    // Transport shared between listener and shipper for a consistent listening interface
    transport: Arc<T>,
    // Default window size for dag transfers
    window_size: u32,
    // Current connection status
    connected: Arc<Mutex<bool>>,
}

impl<T: Transport + Send + 'static> Shipper<T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage_path: &str,
        receiver: Receiver<(DataProtocol, String)>,
        sender: Sender<(DataProtocol, String)>,
        retry_timeout_duration: u64,
        window_size: u32,
        transport: Arc<T>,
        connected: Arc<Mutex<bool>>,
        block_size: u32,
    ) -> Result<Shipper<T>> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider), block_size));
        Ok(Shipper {
            storage,
            window_sessions: BTreeMap::new(),
            receiver,
            sender,
            retry_timeout_duration,
            window_size,
            transport,
            connected,
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
            DataProtocol::RequestTransmitBlock { cid, target_addr } => {
                if *self.connected.lock().unwrap() {
                    self.transmit_block(&cid, &target_addr)?;
                }
            }
            DataProtocol::Block(block) => self.receive_block(block)?,
            DataProtocol::RequestTransmitDag {
                cid,
                target_addr,
                retries,
            } => {
                self.begin_dag_window_session(&cid, &target_addr, retries)?;
            }
            DataProtocol::RetryDagSession { cid } => {
                if *self.connected.lock().unwrap() {
                    if let Some(session) = self.window_sessions.get(&cid) {
                        info!(
                            "Received retry dag session for {cid}, sending get missing req to {}",
                            &session.target_addr
                        );
                        let target_addr = session.target_addr.clone();
                        self.dag_window_session_run(&cid, session.window_num, &target_addr)?;
                        self.retry_dag_window_session(&cid);
                    }
                }
            }
            DataProtocol::RequestMissingDagWindowBlocks { cid, blocks } => {
                if *self.connected.lock().unwrap() {
                    let missing_blocks_msg = handlers::get_missing_dag_blocks_window_protocol(
                        &cid,
                        blocks,
                        Rc::clone(&self.storage),
                    )?;
                    self.transmit_msg(missing_blocks_msg, sender_addr)?;
                }
            }
            DataProtocol::RequestMissingDagBlocks { cid } => {
                if *self.connected.lock().unwrap() {
                    let missing_blocks_msg =
                        handlers::get_missing_dag_blocks(&cid, Rc::clone(&self.storage))?;
                    self.transmit_msg(missing_blocks_msg, &sender_addr)?;
                }
            }
            DataProtocol::MissingDagBlocks { cid, blocks } => {
                if *self.connected.lock().unwrap() {
                    let target_addr = if let Some(session) = self.window_sessions.get(&cid) {
                        session.target_addr.to_string()
                    } else {
                        sender_addr.to_string()
                    };
                    // If no blocks are missing, then attempt to move to next window
                    if blocks.is_empty() {
                        self.increment_dag_window_session(&cid, &target_addr)?;
                    } else {
                        info!(
                            "Dag {cid} is missing {} blocks, sending again",
                            blocks.len()
                        );
                        for b in blocks.clone() {
                            self.transmit_block(&b, &target_addr)?;
                        }
                        self.transmit_msg(
                            Message::DataProtocol(DataProtocol::RequestMissingDagWindowBlocks {
                                cid,
                                blocks,
                            }),
                            &target_addr,
                        )?;
                    }
                }
            }
            DataProtocol::ResumeTransmitDag { cid } => {
                if *self.connected.lock().unwrap() {
                    info!("Shipper resume {cid}");
                    self.resume_dag_window_session(&cid)?;
                }
            }
            DataProtocol::ResumeTransmitAllDags => {
                if *self.connected.lock().unwrap() {
                    self.resume_all_dag_window_sessions()?;
                }
            }
        }
        Ok(())
    }

    // Helper function for adding a new session to the session list
    fn open_dag_window_session(&mut self, cid: &str, retries: u8, target_addr: &str) {
        self.window_sessions
            .entry(cid.to_string())
            .or_insert(WindowSession {
                max_retries: retries,
                remaining_window_retries: retries,
                window_num: 0,
                target_addr: target_addr.to_string(),
            });
    }

    // Helper function for incrementing a session's window and resetting the retries
    fn next_dag_window_session(&mut self, cid: &str) -> Option<u32> {
        if let Some(session) = self.window_sessions.get_mut(cid) {
            session.window_num += 1;
            session.remaining_window_retries = session.max_retries;
            return Some(session.window_num);
        }
        None
    }

    // Helper function for removing sessions which are complete
    fn end_dag_window_session(&mut self, cid: &str) {
        self.window_sessions.remove(cid);
    }

    fn start_dag_window_retry_timeout(&mut self, cid: &str) {
        let sender_clone = self.sender.clone();
        let cid_str = cid.to_string();
        info!("Starting retry timer at {}", self.retry_timeout_duration);
        let timeout_duration = Duration::from_millis(self.retry_timeout_duration);
        spawn(move || {
            sleep(timeout_duration);
            sender_clone
                .send((
                    DataProtocol::RetryDagSession { cid: cid_str },
                    "127.0.0.1:0".to_string(),
                ))
                .unwrap();
        });
    }

    fn retry_dag_window_session(&mut self, cid: &str) {
        if let Some(session) = self.window_sessions.get_mut(cid) {
            if session.remaining_window_retries > 0 {
                session.remaining_window_retries -= 1;
                self.start_dag_window_retry_timeout(cid);
            }
        }
    }

    fn dag_window_session_run(
        &mut self,
        cid: &str,
        window_num: u32,
        target_addr: &str,
    ) -> Result<()> {
        if *self.connected.lock().unwrap() {
            let blocks = self.transmit_dag_window(cid, window_num, target_addr)?;
            if !blocks.is_empty() {
                info!(
                    "Transmitted window {} for {}, {} blocks",
                    window_num,
                    cid,
                    blocks.len()
                );
                self.transmit_msg(
                    Message::DataProtocol(DataProtocol::RequestMissingDagWindowBlocks {
                        cid: cid.to_string(),
                        blocks,
                    }),
                    target_addr,
                )?;
            } else {
                info!("Dag transfer session for {cid} is complete");
                self.end_dag_window_session(cid);
                self.transmit_msg(
                    Message::ApplicationAPI(messages::ApplicationAPI::DagTransmissionComplete {
                        cid: cid.to_string(),
                    }),
                    target_addr,
                )?;
            }
        }
        Ok(())
    }

    // This function resumes the transmission of a DAG by fetching the relevant session
    // and running the last sent window again
    fn resume_dag_window_session(&mut self, cid: &str) -> Result<()> {
        if *self.connected.lock().unwrap() {
            let session = if let Some(session) = self.window_sessions.get(cid) {
                session.clone()
            } else {
                info!("session not found for {cid}");
                return Ok(());
            };
            info!("start dag window session for {cid}");
            // Need to reset the window retries here
            self.dag_window_session_run(cid, session.window_num, &session.target_addr)?;
            self.start_dag_window_retry_timeout(cid);
        }

        Ok(())
    }

    fn resume_all_dag_window_sessions(&mut self) -> Result<()> {
        let session_cids: Vec<String> = self.window_sessions.keys().map(|s| s.to_owned()).collect();
        for cid in session_cids {
            self.resume_dag_window_session(&cid)?;
        }

        Ok(())
    }

    pub fn begin_dag_window_session(
        &mut self,
        cid: &str,
        target_addr: &str,
        retries: u8,
    ) -> Result<()> {
        if *self.connected.lock().unwrap() {
            self.dag_window_session_run(cid, 0, target_addr)?;
            let retries = if retries == 0 { 0 } else { retries - 1 };
            self.open_dag_window_session(cid, retries, target_addr);
            self.start_dag_window_retry_timeout(cid);
        } else {
            self.open_dag_window_session(cid, retries, target_addr);
        }

        Ok(())
    }

    pub fn increment_dag_window_session(&mut self, cid: &str, target_addr: &str) -> Result<()> {
        if *self.connected.lock().unwrap() {
            if let Some(next_window_num) = self.next_dag_window_session(cid) {
                self.dag_window_session_run(cid, next_window_num, target_addr)?;
            }
        }

        Ok(())
    }

    fn transmit_msg(&mut self, msg: Message, target_addr: &str) -> Result<()> {
        let resolved_target_addr = target_addr
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Failed to parse target address"))?;
        info!("Transmitting {msg:?} to {resolved_target_addr}");
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
        if *self.connected.lock().unwrap() {
            let block = self.storage.get_block_by_cid(cid)?;
            self.transmit_blocks(&[block], target_addr)?;
        }
        Ok(())
    }

    pub fn transmit_dag(&mut self, cid: &str, target_addr: &str) -> Result<()> {
        if *self.connected.lock().unwrap() {
            let root_block = self.storage.get_block_by_cid(cid)?;
            let blocks = self.storage.get_all_dag_blocks(cid)?;
            let mut all_blocks = vec![root_block];
            all_blocks.extend(blocks);
            self.transmit_blocks(&all_blocks, target_addr)?;
        }
        Ok(())
    }

    pub fn transmit_dag_window(
        &mut self,
        cid: &str,
        window_num: u32,
        target_addr: &str,
    ) -> Result<Vec<String>> {
        if *self.connected.lock().unwrap() {
            let mut transmitted_cids = vec![];

            let window_blocks = self.get_dag_window_blocks(cid, window_num)?;

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
        } else {
            Ok(vec![])
        }
    }

    fn get_dag_window_blocks(&mut self, cid: &str, window_num: u32) -> Result<Vec<StoredBlock>> {
        let blocks = self
            .storage
            .get_dag_blocks_by_window(cid, self.window_size, window_num)?;
        Ok(blocks)
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
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use transports::UdpTransport;

    struct TestShipper {
        listen_addr: String,
        listen_transport: Arc<UdpTransport>,
        _storage: Rc<Storage>,
        shipper: Shipper<UdpTransport>,
        test_dir: TempDir,
    }

    const BLOCK_SIZE: u32 = 1024 * 3;

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
            let _storage = Rc::new(Storage::new(Box::new(provider), BLOCK_SIZE));
            let (shipper_sender, shipper_receiver) = mpsc::channel();

            let shipper = Shipper::new(
                db_path.to_str().unwrap(),
                shipper_receiver,
                shipper_sender,
                10,
                5,
                shipper_transport,
                Arc::new(Mutex::new(true)),
                BLOCK_SIZE,
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
