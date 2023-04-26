use anyhow::{anyhow, bail, Result};
use messages::{
    ApplicationAPI, DataProtocol, Message, MessageChunker, SimpleChunker, TransmissionBlock,
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
    pub fn new(myceli_address: &str, listen_address: &str, mtu: u16) -> Result<Self> {
        let socket_addr = listen_address
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Failed to parse listen address"))?;
        let socket = Rc::new(UdpSocket::bind(socket_addr)?);
        socket.set_read_timeout(Some(Duration::from_millis(500)))?;
        let address = myceli_address
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Failed to parse myceli address"))?;
        Ok(MyceliApi {
            address,
            socket,
            listen_address: listen_address.to_string(),
            mtu,
        })
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
