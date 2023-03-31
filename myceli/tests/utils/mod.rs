use super::*;

use anyhow::{bail, Result};
use assert_fs::fixture::ChildPath;
use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
use blake2::{Blake2s256, Digest};
use file_hashing::get_hash_file;
use messages::{MessageChunker, SimpleChunker};
use myceli::listener::Listener;
use rand::{thread_rng, Rng, RngCore};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::thread::{sleep, spawn};

pub struct TestListener {
    pub listen_addr: String,
    pub test_dir: TempDir,
}

impl TestListener {
    pub fn new() -> TestListener {
        let test_dir = TempDir::new().unwrap();
        let mut rng = thread_rng();
        let port_num = rng.gen_range(6000..9000);
        let listen_addr = format!("127.0.0.1:{port_num}");

        TestListener {
            listen_addr,
            test_dir,
        }
    }

    pub fn start(&self) -> Result<()> {
        let thread_listen_addr = self
            .listen_addr
            .to_owned()
            .to_socket_addrs()
            .map(|mut i| i.next().unwrap())
            .unwrap();
        let thread_db_path = self.test_dir.child("storage.db");

        spawn(move || start_listener_thread(thread_listen_addr, thread_db_path));

        // A little wait so the listener can get listening
        sleep(Duration::from_millis(50));
        Ok(())
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

fn start_listener_thread(listen_addr: SocketAddr, db_path: ChildPath) {
    let db_path = db_path.path().to_str().unwrap();
    let mut listener = Listener::new(&listen_addr, db_path).unwrap();
    listener.start(10).expect("Error encountered in listener");
}

pub struct TestController {
    pub socket: UdpSocket,
    pub chunker: SimpleChunker,
}

impl TestController {
    pub fn new() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(10)))
            .unwrap();
        let chunker = SimpleChunker::new(60);
        TestController { socket, chunker }
    }

    pub fn send_and_recv(&mut self, target_addr: &str, message: Message) -> Message {
        self.send_msg(message, target_addr);
        self.recv_msg().unwrap()
    }

    pub fn send_msg(&self, message: Message, target_addr: &str) {
        for chunk in self.chunker.chunk(message).unwrap() {
            self.socket.send_to(&chunk, target_addr).unwrap();
        }
    }

    pub fn recv_msg(&mut self) -> Result<Message> {
        let mut tries = 0;
        loop {
            let mut buf = vec![0; 128];
            if self.socket.recv_from(&mut buf).is_ok() {
                match self.chunker.unchunk(&buf) {
                    Ok(Some(msg)) => return Ok(msg),
                    Ok(None) => {}
                    Err(e) => bail!("Error found {e:?}"),
                }
            }
            sleep(Duration::from_millis(10));
            tries += 1;
            if tries > 10 {
                bail!("Listen tries exceeded");
            }
        }
    }
}

pub fn hash_file(path_str: &str) -> String {
    let path = PathBuf::from(path_str);
    let mut hash = Blake2s256::new();
    get_hash_file(path, &mut hash).unwrap()
}
