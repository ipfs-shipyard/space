use crate::udp_chunking::{SimpleChunker, UnchunkResult};
use crate::Transport;
use anyhow::{anyhow, bail, Result};
use messages::Message;
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use tracing::{error};

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
        let mut sender_addr = None;
        let mut read_attempts = 0;
        let mut receive_before_unchunk_tries = 0;
        let mut has_received = false;
        loop {
            loop {
                // Every 100 reads, check for missing chunks in our local recv cache
                // and send out messages requesting those missing chunks
                if has_received && receive_before_unchunk_tries > 100 {
                    receive_before_unchunk_tries = 0;
                    if let Some(sender_addr) = sender_addr {
                        let missing_chunks = self
                            .chunker
                            .lock()
                            .expect("Lock failed, this is bad")
                            .find_missing_chunks()?;
                        if !missing_chunks.is_empty() {
                            for msg in missing_chunks {
                                self.socket.send_to(&msg, sender_addr)?;
                            }
                        }
                    }
                }
                read_attempts += 1;
                match self.socket.recv_from(&mut buf) {
                    Ok((len, sender)) => {
                        if len > 0 {
                            sender_addr = Some(sender);
                            has_received = true;
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

            if let Some(sender_addr) = sender_addr {
                let unchunking_resp = self
                    .chunker
                    .lock()
                    .expect("Lock failed, this is bad")
                    .unchunk(&buf);

                match unchunking_resp {
                    Ok(Some(UnchunkResult::Message(msg))) => {
                        return Ok((msg, sender_addr.to_string()));
                    }
                    Ok(Some(UnchunkResult::Missing(missing))) => {
                        let missing_chunks = self
                            .chunker
                            .lock()
                            .expect("Lock failed, this is bad")
                            .get_prev_sent_chunks(missing.0)?;
                        for chunk in missing_chunks {
                            self.socket.send_to(&chunk, sender_addr)?;
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        bail!("Error unchunking message: {err}");
                    }
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
