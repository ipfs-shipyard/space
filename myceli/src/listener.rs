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

use crate::handlers;
use crate::receiver::Receiver;
use messages::{ApplicationAPI, Message, MessageChunker, SimpleChunker};

// TODO: Make this configurable
pub const MTU: u16 = 60; // 60 for radio

pub struct Listener {
    storage: Rc<Storage>,
    sender_addr: Option<SocketAddr>,
    chunker: SimpleChunker,
    receiver: Receiver,
    socket: Rc<UdpSocket>,
}

impl Listener {
    pub async fn new(listen_address: &str, storage_path: &str) -> Result<Self> {
        let provider = SqliteStorageProvider::new(storage_path)?;
        provider.setup()?;
        let storage = Rc::new(Storage::new(Box::new(provider)));
        let std_socket = std::net::UdpSocket::bind(&listen_address)?;
        let socket = UdpSocket::from_std(std_socket)?;
        let receiver = Receiver::new(Rc::clone(&storage));
        Ok(Listener {
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
                    match self.socket.recv_from(&mut buf).await {
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
                    sleep(Duration::from_millis(10)).await;
                }
            }

            match self.chunker.unchunk(&buf) {
                Ok(Some(msg)) => match self.handle_message(msg).await {
                    Ok(Some(resp)) => self.transmit_response(resp).await.unwrap(),
                    Ok(None) => {}
                    Err(e) => error!("{e}"),
                },
                Ok(None) => {
                    debug!("No msg found yet");
                }
                Err(err) => {
                    error!("Message parse failed: {err}");
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Message) -> Result<Option<Message>> {
        info!("Handling {message:?}");
        let resp = match message {
            Message::ApplicationAPI(ApplicationAPI::TransmitFile { path, target_addr }) => {
                handlers::transmit_file(&path, &target_addr, self.storage.clone()).await?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitDag { cid, target_addr }) => {
                handlers::transmit_dag(&cid, &target_addr, self.storage.clone()).await?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::TransmitBlock { cid, target_addr }) => {
                handlers::transmit_block(&cid, &target_addr, self.storage.clone()).await?;
                None
            }
            Message::ApplicationAPI(ApplicationAPI::ImportFile { path }) => {
                Some(handlers::import_file(&path, self.storage.clone()).await?)
            }
            Message::ApplicationAPI(ApplicationAPI::ExportDag { cid, path }) => {
                self.storage.export_cid(&cid, &PathBuf::from(path)).await?;
                None
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
                self.receiver.handle_transmission_msg(data_msg).await?;
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

    async fn transmit_response(&self, message: Message) -> Result<()> {
        if let Some(sender_addr) = self.sender_addr {
            for chunk in self.chunker.chunk(message)? {
                self.socket.send_to(&chunk, sender_addr).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use anyhow::bail;
    use assert_fs::fixture::ChildPath;
    use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
    use rand::{thread_rng, RngCore};
    use std::thread;
    use tokio::time::sleep;

    struct TestListener {
        listen_addr: String,
        db_dir: TempDir,
    }

    impl TestListener {
        pub fn new(listen_addr: &str) -> TestListener {
            let db_dir = TempDir::new().unwrap();

            return TestListener {
                listen_addr: listen_addr.to_string(),
                db_dir,
            };
        }

        pub async fn start(&self) -> Result<()> {
            let thread_listen_addr = self.listen_addr.to_owned();
            let thread_db_path = self.db_dir.child("storage.db");

            thread::spawn(move || start_listener_thread(thread_listen_addr, thread_db_path));

            // A little wait so the listener can get listening
            sleep(Duration::from_millis(50)).await;
            Ok(())
        }

        pub fn generate_file(&self) -> Result<String> {
            let mut data = Vec::<u8>::new();
            data.resize(256, 1);
            thread_rng().fill_bytes(&mut data);

            let tmp_file = self.db_dir.child("test.file");
            tmp_file.write_binary(&data)?;
            Ok(tmp_file.path().to_str().unwrap().to_owned())
        }
    }

    #[tokio::main]
    async fn start_listener_thread(listen_addr: String, db_path: ChildPath) {
        let db_path = db_path.path().to_str().unwrap();
        let mut listener = Listener::new(&listen_addr, &db_path).await.unwrap();
        listener
            .listen()
            .await
            .expect("Error encountered in listener");
    }

    struct TestController {
        socket: UdpSocket,
        chunker: SimpleChunker,
    }

    impl TestController {
        pub async fn new() -> Self {
            let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let chunker = SimpleChunker::new(60);
            TestController { socket, chunker }
        }

        pub async fn send_and_recv(
            &mut self,
            target_addr: &str,
            message: Message,
        ) -> Result<Message> {
            self.send_msg(message, target_addr).await?;
            self.recv_msg().await
        }

        pub async fn send_msg(&self, message: Message, target_addr: &str) -> Result<()> {
            for chunk in self.chunker.chunk(message).unwrap() {
                self.socket.send_to(&chunk, target_addr).await.unwrap();
            }
            Ok(())
        }

        pub async fn recv_msg(&mut self) -> Result<Message> {
            let mut tries = 0;
            loop {
                let mut buf = vec![0; 128];
                if self.socket.try_recv_from(&mut buf).is_ok() {
                    match self.chunker.unchunk(&buf) {
                        Ok(Some(msg)) => return Ok(msg),
                        Ok(None) => {}
                        Err(e) => bail!("Error found {e:?}"),
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
                tries += 1;
                if tries > 5 {
                    bail!("Listen tries exceeded");
                }
            }
        }
    }

    #[tokio::test]
    pub async fn test_verify_listener_alive() {
        let listener = TestListener::new("127.0.0.1:8080");
        listener.start().await.unwrap();

        let mut controller = TestController::new().await;

        let response = controller
            .send_and_recv(&listener.listen_addr, Message::request_available_blocks())
            .await
            .unwrap();

        assert_eq!(response, Message::available_blocks(vec![]));
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_transmit_receive_block() {
        let transmitter = TestListener::new("127.0.0.1:8080");
        let receiver = TestListener::new("127.0.0.1:8081");
        let mut controller = TestController::new().await;

        transmitter.start().await.unwrap();
        receiver.start().await.unwrap();

        let test_file_path = transmitter.generate_file().unwrap();
        let resp = controller
            .send_and_recv(
                &transmitter.listen_addr,
                Message::import_file(&test_file_path),
            )
            .await
            .unwrap();
        let root_cid = match resp {
            Message::ApplicationAPI(ApplicationAPI::FileImported { cid, .. }) => cid,
            other => panic!("Failed to receive FileImported msg {other:?}"),
        };

        controller
            .send_msg(
                Message::transmit_block(&root_cid, &receiver.listen_addr),
                &transmitter.listen_addr,
            )
            .await
            .unwrap();

        sleep(Duration::from_millis(100)).await;

        let resp = controller
            .send_and_recv(&receiver.listen_addr, Message::request_available_blocks())
            .await
            .unwrap();

        assert_eq!(resp, Message::available_blocks(vec![root_cid]));
    }
}
