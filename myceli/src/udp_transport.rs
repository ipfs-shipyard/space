use crate::transport::Transport;
use anyhow::{bail, Result};
use messages::{Message, MessageChunker, SimpleChunker};
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, error};

pub struct UdpTransport {
    socket: UdpSocket,
    mtu: u16,
    chunker: Arc<Mutex<SimpleChunker>>,
}

impl Transport for UdpTransport {
    fn new(listen_addr: &str, mtu: u16) -> Result<Self> {
        let socket = UdpSocket::bind(listen_addr)?;
        Ok(UdpTransport {
            mtu,
            socket,
            chunker: Arc::new(Mutex::new(SimpleChunker::new(mtu))),
        })
    }

    fn receive(&self) -> Result<(Message, String)> {
        let mut buf = vec![0; usize::from(self.mtu)];
        let mut sender_addr;
        loop {
            loop {
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
                sleep(Duration::from_millis(10));
            }

            match self.chunker.lock().unwrap().unchunk(&buf) {
                Ok(Some(msg)) => return Ok((msg, sender_addr.to_string())),
                Ok(None) => debug!("No msg yet"),
                Err(err) => {
                    bail!("Error unchunking message: {err}");
                }
            }
        }
    }

    fn send(&self, msg: Message, addr: &str) -> Result<()> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        for chunk in self.chunker.lock().unwrap().chunk(msg)? {
            self.socket.send_to(&chunk, addr)?;
        }
        Ok(())
    }
}
