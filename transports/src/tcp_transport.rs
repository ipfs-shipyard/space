use crate::udp_chunking::SimpleChunker;
use crate::Transport;
use anyhow::{anyhow, bail, Result};
use messages::Message;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, error, info};

pub struct TcpTransport {
    pub socket: TcpListener,
    mtu: u16,
    chunker: Arc<Mutex<SimpleChunker>>,
    max_read_attempts: Option<u16>,
}

impl TcpTransport {
    pub fn new(listen_addr: &str, mtu: u16) -> Result<Self> {
        let socket = TcpListener::bind(listen_addr)?;
        Ok(TcpTransport {
            mtu,
            socket,
            chunker: Arc::new(Mutex::new(SimpleChunker::new(mtu))),
            max_read_attempts: None,
        })
    }

    pub fn set_max_read_attempts(&mut self, attempts: Option<u16>) {
        self.max_read_attempts = attempts;
    }
}

impl Transport for TcpTransport {
    fn receive(&self) -> Result<(Message, String)> {
        let mut buf = vec![0; usize::from(self.mtu)];
        let mut sender_addr;

        loop {
            loop {
                match self.socket.accept() {
                    Ok((mut stream, sender)) => match stream.read(&mut buf) {
                        Ok(len) => {
                            if len > 0 {
                                sender_addr = sender;
                                break;
                            }
                        }
                        Err(e) => {
                            debug!("Recv failed {e}")
                        }
                    },
                    Err(e) => {
                        debug!("Failed to accept connection: {e}");
                    }
                }
                sleep(Duration::from_millis(10));
            }

            match self
                .chunker
                .lock()
                .expect("Lock failed, this is bad")
                .unchunk(&buf)
            {
                Ok(Some(msg)) => return Ok((msg, sender_addr.to_string())),
                Ok(None) => {}
                Err(e) => {
                    bail!("Failed unchunking msg: {e}");
                }
            }
        }
    }

    fn send(&self, msg: Message, addr: &str) -> Result<()> {
        let mut stream = TcpStream::connect(addr)?;
        for chunk in self
            .chunker
            .lock()
            .expect("Lock failed, this is really bad")
            .chunk(msg)?
        {
            stream.write(&chunk)?;
        }
        Ok(())
    }
}
