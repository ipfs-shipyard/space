use crate::udp_chunking::SimpleChunker;
use crate::Transport;
use anyhow::{anyhow, bail, Result};
use messages::Message;
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, error};

pub struct UdpTransport {
    pub socket: UdpSocket,
    mtu: u16,
    chunker: Arc<Mutex<SimpleChunker>>,
    max_read_attempts: Option<u16>,
}

impl UdpTransport {
    pub fn new(listen_addr: &str, mtu: u16) -> Result<Self> {
        let socket = UdpSocket::bind(listen_addr)?;
        Ok(UdpTransport {
            mtu,
            socket,
            chunker: Arc::new(Mutex::new(SimpleChunker::new(mtu))),
            max_read_attempts: None,
        })
    }

    pub fn set_read_timeout(&mut self, dur: Option<Duration>) -> Result<()> {
        Ok(self.socket.set_read_timeout(dur)?)
    }

    pub fn set_max_read_attempts(&mut self, attempts: Option<u16>) {
        self.max_read_attempts = attempts;
    }
}

impl Transport for UdpTransport {
    fn receive(&self) -> Result<(Message, String)> {
        let mut buf = vec![0; usize::from(self.mtu)];
        let mut sender_addr;
        let mut read_attempts = 0;
        loop {
            loop {
                read_attempts += 1;
                match self.socket.recv_from(&mut buf) {
                    Ok((len, sender)) => {
                        if len > 0 {
                            sender_addr = sender;
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Recv failed {e}");
                    }
                }
                if let Some(max_attempts) = self.max_read_attempts {
                    if read_attempts > max_attempts {
                        bail!("Exceeded number of read attempts");
                    }
                }
                sleep(Duration::from_millis(10));
            }

            match self
                .chunker
                .lock()
                .expect("Lock failed, this is really bad")
                .unchunk(&buf)
            {
                Ok(Some(msg)) => return Ok((msg, sender_addr.to_string())),
                Ok(None) => debug!("No msg yet"),
                Err(err) => {
                    bail!("Error unchunking message: {err}");
                }
            }
        }
    }

    fn send(&self, msg: Message, addr: &str) -> Result<()> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Failed to parse address"))?;
        for chunk in self
            .chunker
            .lock()
            .expect("Lock failed, this is really bad")
            .chunk(msg)?
        {
            self.socket.send_to(&chunk, addr)?;
        }
        Ok(())
    }
}
