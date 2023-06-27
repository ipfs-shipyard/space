use anyhow::Result;
use assert_fs::fixture::ChildPath;
use assert_fs::{fixture::FileWriteBin, fixture::PathChild, TempDir};
use blake2::{Blake2s256, Digest};
use file_hashing::get_hash_file;
use messages::Message;
use myceli::listener::Listener;
use rand::{thread_rng, Rng, RngCore};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use transports::{Transport, UdpTransport};

pub fn testing_setup() -> (TestListener, TestListener, TestController) {
    let transmitter = TestListener::new();
    let receiver = TestListener::new();
    let controller = TestController::new();
    (transmitter, receiver, controller)
}

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
        sleep(Duration::from_millis(100));
    }
}

pub struct TestListener {
    pub listen_addr: String,
    pub test_dir: TempDir,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    db_path: ChildPath,
}

impl TestListener {
    pub fn new() -> TestListener {
        let test_dir = TempDir::new().unwrap();
        let mut rng = thread_rng();
        let port_num = rng.gen_range(6000..9000);
        let listen_addr = format!("127.0.0.1:{port_num}");
        let db_path = test_dir.child("storage.db");

        TestListener {
            listen_addr,
            test_dir,
            db_path,
            thread_handle: None,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.thread_handle.is_none() {
            let thread_listen_addr = self
                .listen_addr
                .to_owned()
                .to_socket_addrs()
                .map(|mut i| i.next().unwrap())
                .unwrap();

            let thread_db_path = self.db_path.to_owned();

            self.thread_handle = Some(spawn(move || {
                start_listener_thread(thread_listen_addr, thread_db_path)
            }));

            // A little wait so the listener can get listening
            sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.thread_handle.take() {
            handle.join().unwrap();
        }
    }

    pub fn generate_file(&self, size: u32) -> Result<String> {
        let mut data = Vec::<u8>::new();
        data.resize(size as usize, 1);
        thread_rng().fill_bytes(&mut data);

        let tmp_file = self.test_dir.child("test.file");
        tmp_file.write_binary(&data)?;
        Ok(tmp_file.path().to_str().unwrap().to_owned())
    }
}

fn start_listener_thread(listen_addr: SocketAddr, db_path: PathBuf) {
    let db_path = db_path.to_str().unwrap();
    let listen_addr_str = listen_addr.to_string();
    let mut transport = UdpTransport::new(&listen_addr_str, 512, None).unwrap();
    transport
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    transport.set_max_read_attempts(Some(10));
    let transport = Arc::new(transport);
    let mut listener = Listener::new(&listen_addr, db_path, transport, 1024 * 3, None).unwrap();
    listener
        .start(10, 5, 1024 * 3)
        .expect("Error encountered in listener");
}

pub struct TestController {
    pub transport: UdpTransport,
}

impl TestController {
    pub fn new() -> Self {
        let mut transport = UdpTransport::new("127.0.0.1:0", 512, None).unwrap();
        transport
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        transport.set_max_read_attempts(Some(1));
        TestController { transport }
    }

    pub fn send_and_recv(&mut self, target_addr: &str, message: Message) -> Message {
        println!("\n\t#\tSending a msg to {}, will expect a response: {:?}\n", target_addr, &message);
        self.send_msg(message, target_addr);
        let mut retries = 0;
        loop {
            if let Ok(msg) = self.recv_msg() {
                println!("\t#\tGot the expected response: {:?}\n\n", &msg);
                return msg;
            }
            if retries > 50 {
                panic!("Send recv failed");
            }
            retries += 1;
            sleep(Duration::from_secs(1));
        }
    }

    pub fn send_msg(&self, message: Message, target_addr: &str) {
        self.transport
            .send(message, target_addr)
            .expect("Transport send failed");
    }

    pub fn recv_msg(&mut self) -> Result<Message> {
        let (msg, _) = self.transport.receive()?;
        Ok(msg)
    }
}

pub fn hash_file(path_str: &str) -> String {
    let path = PathBuf::from(path_str);
    let mut hash = Blake2s256::new();
    get_hash_file(path, &mut hash).unwrap()
}
