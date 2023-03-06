use anyhow::Result;
use local_storage::provider::SqliteStorageProvider;
use local_storage::storage::Storage;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use tracing::{debug, error, info};

use crate::receive::receive;
use crate::receiver::Receiver;
use crate::transmit::transmit_blocks;
use messages::{ApplicationAPI, Message, MessageChunker, SimpleChunker};

// TODO: Make this configurable
pub const MTU: u16 = 60; // 60 for radio

pub struct Server {
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    chunker: SimpleChunker,
    receiver: Receiver,
    socket: Rc<UdpSocket>,
}

impl Server {
    pub async fn new(listen_address: &str) -> Result<Self> {
        let provider = SqliteStorageProvider::new("storage.db")?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        let socket = UdpSocket::bind(&listen_address).await?;
        info!("Listening for messages on {}", listen_address);
        let receiver = Receiver::new(Rc::clone(&storage));
        Ok(Server {
            storage,
            sender_addr: None,
            chunker: SimpleChunker::new(MTU),
            receiver,
            socket: Rc::new(socket),
        })
    }

    pub async fn listen(&mut self) -> Result<()> {
        let mut buf = vec![0; usize::from(MTU)];
        loop {
            {
                loop {
                    if let Ok((len, sender)) = self.socket.try_recv_from(&mut buf) {
                        if len > 0 {
                            self.sender_addr = Some(sender);
                            break;
                        }
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }

            match self.chunker.unchunk(&buf) {
                Ok(Some(msg)) => {
                    if let Err(e) = self.handle_message(msg).await {
                        error!("{e}");
                    }
                }
                Ok(None) => {
                    debug!("No msg found yet");
                }
                Err(err) => {
                    error!("{err}");
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Message) -> Result<()> {
        match message {
            Message::ApplicationAPI(ApplicationAPI::Receive { listen_addr }) => {
                receive(&listen_addr).await?
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitFile { path, target_addr }) => {
                let cid = self
                    .storage
                    .import_path(&PathBuf::from(path.to_owned()))
                    .await?;
                let root_block = self.storage.get_block_by_cid(&cid)?;
                let blocks = self.storage.get_all_blocks_under_cid(&cid)?;
                let mut all_blocks = vec![root_block];
                all_blocks.extend(blocks);
                transmit_blocks(&all_blocks, &target_addr).await?
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitDag { cid, target_addr }) => {
                let root_block = self.storage.get_block_by_cid(&cid)?;
                let blocks = self.storage.get_all_blocks_under_cid(&cid)?;
                let mut all_blocks = vec![root_block];
                all_blocks.extend(blocks);
                transmit_blocks(&all_blocks, &target_addr).await?
            }
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path }) => {
                if let Some(resp) = import_file(&path, self.storage.clone()).await? {
                    self.transmit_response(resp).await?;
                }
            }
            Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path }) => {
                self.storage.export_cid(&cid, &PathBuf::from(path)).await?
            }
            Message::ApplicationAPI(ApplicationAPI::RequestAvailableBlocks) => {
                let raw_cids = self.storage.list_available_cids()?;
                let cids = raw_cids
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>();

                if let Some(sender_addr) = self.sender_addr {
                    let response =
                        Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids });
                    for chunk in self.chunker.chunk(response)? {
                        self.socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Message::ApplicationAPI(ApplicationAPI::GetMissingDagBlocks { cid }) => {
                let missing_blocks = self.storage.get_missing_dag_blocks(&cid)?;
                if let Some(sender_addr) = self.sender_addr {
                    let response = Message::ApplicationAPI(ApplicationAPI::MissingDagBlocks {
                        blocks: missing_blocks,
                    });
                    for chunk in self.chunker.chunk(response)? {
                        self.socket.send_to(&chunk, sender_addr).await?;
                    }
                }
            }
            Message::ApplicationAPI(ApplicationAPI::ValidateDag { cid }) => {
                if let Some(resp) = validate_dag(&cid, self.storage.clone()).await? {
                    self.transmit_response(resp).await?;
                }
            }
            Message::DataProtocol(data_msg) => {
                self.receiver.handle_transmission_msg(data_msg).await?;
            }
            // Default case for valid messages which don't have handling code implemented yet
            message => {
                info!("Received unhandled message: {:?}", message);
            }
        }
        Ok(())
    }

    async fn transmit_response(&self, message: Message) -> Result<()> {
        if let Some(sender_addr) = self.sender_addr {
            for chunk in self.chunker.chunk(message)? {
                self.socket.send_to(&chunk, sender_addr).await?;
            }
        }
        Ok(())
    }
}

async fn import_file(path: &str, storage: Rc<Storage>) -> Result<Option<Message>> {
    let root_cid = storage.import_path(&PathBuf::from(path.to_owned())).await?;
    Ok(Some(Message::ApplicationAPI(
        ApplicationAPI::FileImported {
            path: path.to_string(),
            cid: root_cid,
        },
    )))
}

async fn validate_dag(cid: &str, storage: Rc<Storage>) -> Result<Option<Message>> {
    let dag_blocks = storage.get_dag_blocks(cid)?;
    let resp = match local_storage::block::validate_dag(&dag_blocks) {
        Ok(_) => "Dag is valid".to_string(),
        Err(e) => e.to_string(),
    };
    Ok(Some(Message::ApplicationAPI(
        ApplicationAPI::ValidateDagResponse {
            cid: cid.to_string(),
            result: resp,
        },
    )))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
    use rand::{thread_rng, RngCore};

    struct TestHarness {
        storage: Rc<Storage>,
        db_dir: TempDir,
    }

    impl TestHarness {
        pub fn new() -> Self {
            let db_dir = TempDir::new().unwrap();
            let db_path = db_dir.child("storage.db");
            let provider = SqliteStorageProvider::new(db_path.path().to_str().unwrap()).unwrap();
            provider.setup().unwrap();
            let storage = Rc::new(Storage::new(Box::new(provider)));
            return TestHarness { storage, db_dir };
        }

        pub fn generate_file(&self) -> Result<String> {
            let mut data = Vec::<u8>::new();
            data.resize(80, 1);
            thread_rng().fill_bytes(&mut data);

            let tmp_file = self.db_dir.child("test.file");
            tmp_file.write_binary(&data)?;
            Ok(tmp_file.path().to_str().unwrap().to_owned())
        }
    }

    #[tokio::test]
    pub async fn test_import_file_validate_dag() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        match import_file(&test_file_path, harness.storage.clone()).await {
            Ok(Some(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }))) => {
                let resp = validate_dag(&cid, harness.storage.clone())
                    .await
                    .unwrap()
                    .unwrap();
                match resp {
                    Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
                        cid: validated_cid,
                        result,
                    }) => {
                        assert_eq!(cid, validated_cid);
                        assert_eq!(result, "Dag is valid");
                    }
                    m => {
                        panic!("ValidateDag returned wrong response {m:?}");
                    }
                }
            }
            m => {
                panic!("ImportFile returned wrong response {m:?}");
            }
        }
    }

    #[tokio::test]
    pub async fn test_import_file_validate_blocks() {
        let harness = TestHarness::new();

        let test_file_path = harness.generate_file().unwrap();

        match import_file(&test_file_path, harness.storage.clone()).await {
            Ok(Some(Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }))) => {
                let blocks = harness.storage.get_all_blocks_under_cid(&cid).unwrap();
                for block in blocks {
                    match validate_dag(&block.cid, harness.storage.clone())
                        .await
                        .unwrap()
                        .unwrap()
                    {
                        Message::ApplicationAPI(ApplicationAPI::ValidateDagResponse {
                            cid: validated_cid,
                            result,
                        }) => {
                            assert_eq!(block.cid, validated_cid);
                            assert_eq!(result, "Dag is valid");
                        }
                        m => {
                            panic!("ValidateDag returned wrong response {m:?}");
                        }
                    }
                }
            }
            m => {
                panic!("ImportFile returned wrong response {m:?}");
            }
        }
    }
}
