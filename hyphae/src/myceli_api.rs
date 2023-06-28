use anyhow::{bail, Result};
use messages::{ApplicationAPI, DataProtocol, Message, TransmissionBlock};
use std::rc::Rc;
use std::time::Duration;
use tracing::{ warn, debug};
use transports::{Transport, UdpTransport};

pub struct MyceliApi {
    address: String,
    listen_address: String,
    transport: Rc<UdpTransport>,
}

impl MyceliApi {
    pub fn new(
        myceli_address: &str,
        listen_address: &str,
        mtu: u16,
        chunk_transmit_throttle: Option<u32>,
    ) -> Result<Self> {
        let mut transport = Rc::new(UdpTransport::new(
            listen_address,
            mtu,
            chunk_transmit_throttle,
        )?);
        Rc::get_mut(&mut transport).unwrap().set_read_timeout(Some(Duration::from_secs(30))).expect("Error setting read timeout");
        Ok(MyceliApi {
            address: myceli_address.to_string(),
            listen_address: listen_address.to_string(),
            transport,
        })
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        self.transport.send(msg, &self.address)
    }

    fn recv_msg(&self) -> Result<Message> {
        let (msg, _) = self.transport.receive()?;
        Ok(msg)
    }

    pub fn check_alive(&self) -> bool {
        match self
            .send_msg(Message::request_version())
            .and_then(|_| self.recv_msg())
        {
            Ok(Message::ApplicationAPI(ApplicationAPI::Version { version })) => {
                debug!("Found myceli version {version}");
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
