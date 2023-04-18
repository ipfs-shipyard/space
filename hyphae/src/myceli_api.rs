use anyhow::{bail, Result};
use messages::{
    ApplicationAPI, DataProtocol, Message, MessageChunker, SimpleChunker, TransmissionBlock,
    UnchunkResult,
};
use std::net::SocketAddr;
use std::net::{ToSocketAddrs, UdpSocket};
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, info, warn};

pub struct MyceliApi {
    address: SocketAddr,
    socket: Rc<UdpSocket>,
    listen_address: String,
    mtu: u16,
}

impl MyceliApi {
    pub fn new(address: &str, mtu: u16) -> Self {
        let socket = Rc::new(UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket"));
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("Failed to set socket read timeout");
        let listen_address = socket
            .local_addr()
            .expect("Failed to create local address")
            .to_string();
        let address = address
            .to_socket_addrs()
            .ok()
            .and_then(|mut iter| iter.next())
            .expect("Failed to resolve address into socketaddr");
        MyceliApi {
            address,
            socket,
            listen_address,
            mtu,
        }
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        let chunker = SimpleChunker::new(self.mtu);
        for chunk in chunker.chunk(msg)? {
            self.socket.send_to(&chunk, self.address)?;
        }
        Ok(())
    }

    fn recv_msg(&self) -> Result<Message> {
        let mut chunker = SimpleChunker::new(self.mtu);
        let mut buf = vec![0; self.mtu.into()];
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
                Ok(Some(UnchunkResult::Message(msg))) => return Ok(msg),
                Ok(Some(other)) => {}
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

    pub fn check_alive(&self) -> bool {
        match self
            .send_msg(Message::request_version())
            .and_then(|_| self.recv_msg())
        {
            Ok(Message::ApplicationAPI(ApplicationAPI::Version { version })) => {
                info!("Found myceli version {version}");
                true
            }
            Ok(other_msg) => {
                warn!("Myceli returned wrong version message: {other_msg:?}");
                false
            }
            Err(e) => {
                warn!("Could not contact myceli at this time: {e}");
                false
            }
        }
    }

    pub fn get_available_blocks(&self) -> Result<Vec<String>> {
        self.send_msg(Message::request_available_blocks())?;
        match self.recv_msg() {
            Ok(Message::ApplicationAPI(ApplicationAPI::AvailableBlocks { cids })) => Ok(cids),
            other => {
                // TODO: extract out to macro which logs and bails
                warn!("Received wrong resp for RequestAvailableBlocks: {other:?}");
                bail!("Received wrong resp for RequestAvailableBlocks: {other:?}")
            }
        }
    }

    pub fn get_block(&self, cid: &str) -> Result<TransmissionBlock> {
        self.send_msg(Message::transmit_block(cid, &self.listen_address))?;
        match self.recv_msg() {
            Ok(Message::DataProtocol(DataProtocol::Block(block))) => Ok(block),
            other => {
                warn!("Received wrong resp for RequestBlock: {other:?}");
                bail!("Received wrong resp for RequestBlock: {other:?}")
            }
        }
    }
}
