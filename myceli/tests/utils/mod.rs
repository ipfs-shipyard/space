use super::*;

use anyhow::Result;
use assert_fs::fixture::ChildPath;
use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
use blake2::{Blake2s256, Digest};
use file_hashing::get_hash_file;
use myceli::listener::Listener;
use rand::{rngs::StdRng, thread_rng, Rng, RngCore, SeedableRng};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use transports::{Transport, UdpTransport};

const BLOCK_SIZE: u32 = 1024 * 3;

pub fn wait_receiving_done(receiver: &TestListener, controller: &mut TestController) {
    let mut prev_num_blocks = 0;
    let mut num_retries = 0;

    loop {
        let current_blocks =
            if let Message::ApplicationAPI(messages::ApplicationAPI::AvailableBlocks { cids }) =
                controller.send_and_recv(&receiver.listen_addr, Message::request_available_blocks())
            {
                cids
            } else {
                panic!("Failed to get correct response to blocks request");
            };
        let current_num_blocks = current_blocks.len();
        if current_num_blocks > prev_num_blocks {
            prev_num_blocks = current_num_blocks;
            num_retries = 0;
        } else {
            if num_retries > 10 {
                break;
            }
            num_retries += 1;
        }
        sleep(Duration::from_millis(num_retries * num_retries + 1));
    }
}

pub struct TestListener {
    pub listen_addr: String,
    pub test_dir: TempDir,
}

impl TestListener {
    pub fn new() -> TestListener {
        let test_dir = TempDir::new().unwrap();
        let port_num = thread_rng().gen_range(6000..9000);
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
        data.resize(256 * 50, 1);
        let mut rng = StdRng::seed_from_u64(2);
        rng.fill_bytes(&mut data);

        let tmp_file = self.test_dir.child("test.file");
        tmp_file.write_binary(&data)?;
        Ok(tmp_file.path().to_str().unwrap().to_owned())
    }
}

fn start_listener_thread(listen_addr: SocketAddr, db_path: ChildPath) {
    let db_path = db_path.path().to_str().unwrap();
    let listen_addr_str = listen_addr.to_string();
    std::thread::sleep(Duration::from_millis(1));
    let mut transport = UdpTransport::new(&listen_addr_str, 60, None).unwrap();
    transport
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    transport.set_max_read_attempts(Some(1));
    let transport = Arc::new(transport);
    let mut listener =
        Listener::new(&listen_addr, db_path, transport, BLOCK_SIZE, None, 9, 512).unwrap();
    listener
        .start(10, 2, BLOCK_SIZE, 0)
        .expect("Error encountered in listener");
}

pub struct TestController {
    pub transport: UdpTransport,
}

impl TestController {
    pub fn new() -> Self {
        let mut transport = UdpTransport::new("127.0.0.1:0", 60, None).unwrap();
        transport
            .set_read_timeout(Some(Duration::from_millis(9008)))
            .unwrap();
        transport.set_max_read_attempts(Some(1));
        TestController { transport }
    }

    pub fn send_and_recv(&mut self, target_addr: &str, message: Message) -> Message {
        self.send_msg(message, target_addr);
        std::thread::sleep(Duration::from_millis(530));
        self.recv_msg().unwrap()
    }

    pub fn send_msg(&self, message: Message, target_addr: &str) {
        self.transport
            .send(message, target_addr)
            .expect("Transport send failed");
    }

    pub fn recv_msg(&mut self) -> Result<Message> {
        Ok(self.transport.receive()?.0)
    }
}

pub fn hash_file(path_str: &str) -> String {
    let path = PathBuf::from(path_str);
    let mut hash = Blake2s256::new();
    get_hash_file(path, &mut hash).unwrap()
}
