use crate::udp_chunking::SimpleChunker;
use crate::{Transport, MAX_MTU};
use anyhow::{anyhow, bail, Result};
use messages::Message;
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use tracing::debug;

pub struct UdpTransport {
    pub socket: UdpSocket,
    chunker: Arc<Mutex<SimpleChunker>>,
    max_read_attempts: Option<u16>,
    chunk_transmit_throttle: Option<u32>,
}

impl UdpTransport {
    pub fn new(listen_addr: &str, mtu: u16, chunk_transmit_throttle: Option<u32>) -> Result<Self> {
        let socket = UdpSocket::bind(listen_addr)?;
        Ok(UdpTransport {
            socket,
            chunker: Arc::new(Mutex::new(SimpleChunker::new(mtu))),
            max_read_attempts: None,
            chunk_transmit_throttle,
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
        let mut buf = vec![0; usize::from(MAX_MTU)];
        let mut sender_addr;
        let mut read_attempts = 0;
        let mut read_len;
        loop {
            loop {
                read_attempts += 1;
                match self.socket.recv_from(&mut buf) {
                    Ok((len, sender)) => {
                        if len > 0 {
                            read_len = len;
                            sender_addr = sender;
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("Recv failed {e}");
                    }
                }
                if let Some(max_attempts) = self.max_read_attempts {
                    if read_attempts > max_attempts {
                        bail!("Exceeded number of read attempts");
                    }
                }
                sleep(Duration::from_millis(10));
            }

            debug!("Received possible chunk of {} bytes", read_len);
            let hex_str = buf[0..read_len]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<String>();
            debug!("Received possible chunk of hex {hex_str}");

            match self
                .chunker
                .lock()
                .expect("Lock failed, this is really bad")
                .unchunk(&buf[0..read_len])
            {
                Ok(Some(msg)) => {
                    debug!("Assembled msg: {msg:?}");
                    return Ok((msg, sender_addr.to_string()));
                }
                Ok(None) => {
                    debug!("Received: no msg ready for assembly yet");
                }
                Err(err) => {
                    bail!("Error unchunking message: {err}");
                }
            }
        }
    }

    fn send(&self, msg: Message, addr: &str) -> Result<()> {
        debug!("Transmitting msg: {msg:?}");
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
            debug!("Transmitting chunk of {} bytes", chunk.len());
            let hex_str = chunk.iter().map(|b| format!("{b:02X}")).collect::<String>();
            debug!("Transmitting chunk of hex {hex_str}");
            self.socket.send_to(&chunk, addr)?;
            if let Some(throttle) = self.chunk_transmit_throttle {
                sleep(Duration::from_millis(throttle.into()));
            }
        }
        Ok(())
    }
}
