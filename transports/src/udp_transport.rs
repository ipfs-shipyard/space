use crate::error::TransportError;
use crate::{
    error::{adhoc, Result},
    udp_chunking::SimpleChunker,
    Transport, MAX_MTU,
};
use log::{debug, error, info, trace};
use messages::Message;
use std::{
    io,
    net::{ToSocketAddrs, UdpSocket},
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

pub struct UdpTransport {
    pub socket: UdpSocket,
    chunker: Arc<Mutex<SimpleChunker>>,
    max_read_attempts: Option<u16>,
    chunk_transmit_throttle: Option<u32>,
    timeout: Option<Duration>,
}

impl UdpTransport {
    pub fn new(listen_addr: &str, mtu: u16, chunk_transmit_throttle: Option<u32>) -> Result<Self> {
        info!("Will listen on {}", &listen_addr);
        let socket = UdpSocket::bind(listen_addr)?;
        Ok(UdpTransport {
            socket,
            chunker: Arc::new(Mutex::new(SimpleChunker::new(mtu))),
            max_read_attempts: None,
            chunk_transmit_throttle,
            timeout: None,
        })
    }

    pub fn set_read_timeout(&mut self, dur: Option<Duration>) -> Result<()> {
        self.timeout = dur;
        Ok(self.socket.set_read_timeout(dur.map(|d| d / 10))?)
    }

    pub fn set_max_read_attempts(&mut self, attempts: Option<u16>) {
        self.max_read_attempts = attempts;
    }
}

impl Transport for UdpTransport {
    fn receive(&self) -> Result<(Message, String)> {
        let mut buf = vec![0; usize::from(MAX_MTU)];
        let mut sender_addr;
        let mut read_errors = 0;
        let mut read_len;
        let mut timeouts = 0;
        loop {
            loop {
                match self.socket.recv_from(&mut buf) {
                    Ok((len, sender)) => {
                        if len > 0 {
                            read_len = len;
                            sender_addr = sender;
                            break;
                        }
                    }
                    Err(e) => match e.kind() {
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                            trace!("Receive timed out. May be normal depending on usage.");
                            if timeouts >= 10 {
                                return Err(TransportError::TimedOut);
                            }
                            timeouts += 1;
                        }
                        _ => {
                            error!("Recv failed {e}");
                            if self.max_read_attempts.unwrap_or(u16::MAX) <= read_errors {
                                return Err(e.into());
                            }
                            read_errors += 1;
                        }
                    },
                }
                sleep(Duration::from_millis(1));
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
                    return Err(err);
                }
            }
        }
    }

    fn send(&self, msg: Message, addr: &str) -> Result<()> {
        debug!("UDP: Transmitting msg: {msg:?}");
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or(adhoc("Failed to parse address"))?;
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
