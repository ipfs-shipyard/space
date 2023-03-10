use super::*;

use anyhow::{bail, Result};
use assert_fs::fixture::ChildPath;
use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
use messages::{MessageChunker, SimpleChunker};
use myceli::listener::Listener;
use rand::{thread_rng, RngCore};
use std::thread;
use tokio::net::UdpSocket;
use tokio::time::sleep;

pub struct TestListener {
    pub listen_addr: String,
    pub db_dir: TempDir,
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

pub struct TestController {
    pub socket: UdpSocket,
    pub chunker: SimpleChunker,
}

impl TestController {
    pub async fn new() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let chunker = SimpleChunker::new(60);
        TestController { socket, chunker }
    }

    pub async fn send_and_recv(&mut self, target_addr: &str, message: Message) -> Result<Message> {
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
