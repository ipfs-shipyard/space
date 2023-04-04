use anyhow::{bail, Result};
use messages::{ApplicationAPI, Message, MessageChunker, SimpleChunker};
use std::net::{ToSocketAddrs, UdpSocket};
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, info, warn};

pub struct MyceliApi {
    address: String,
    socket: Rc<UdpSocket>,
}

impl MyceliApi {
    pub fn new(address: &str) -> Self {
        let socket = Rc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        socket
            .set_read_timeout(Some(Duration::from_millis(10)))
            .unwrap();
        MyceliApi {
            address: address.to_string(),
            socket,
        }
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        let resolved_target_addr = self.address.to_socket_addrs().unwrap().next().unwrap();

        let chunker = SimpleChunker::new(1024);
        for chunk in chunker.chunk(msg)? {
            self.socket.send_to(&chunk, resolved_target_addr)?;
        }
        Ok(())
    }

    fn recv_msg(&self) -> Result<Message> {
        let mut chunker = SimpleChunker::new(1024);
        let mut buf = vec![0; 1024];
        loop {
            {
                loop {
                    match self.socket.recv_from(&mut buf) {
                        Ok((len, _)) => {
                            if len > 0 {
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Recv failed {e}");
                            bail!("Recv failed {e}");
                        }
                    }
                    sleep(Duration::from_millis(10));
                }
            }

            match chunker.unchunk(&buf) {
                Ok(Some(msg)) => return Ok(msg),
                Ok(None) => {
                    debug!("No msg found yet")
                }
                Err(err) => {
                    warn!("Message parsed failed: {err}");
                    bail!("Message parse failed: {err}")
                }
            }
        }
    }

    pub fn check_alive(&self) -> Result<()> {
        self.send_msg(Message::request_version())?;
        if let Ok(Message::ApplicationAPI(ApplicationAPI::Version { version })) = self.recv_msg() {
            info!("Found myceli version {version}");
            Ok(())
        } else {
            bail!("Recv message failed");
        }
    }
}
