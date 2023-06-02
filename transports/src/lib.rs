mod chunking;
mod udp_chunking;
mod udp_transport;

use anyhow::Result;
use messages::Message;

pub const MAX_MTU: u16 = 1024 * 3;

pub trait Transport: Send + Sync {
    fn receive(&self) -> Result<(Message, String)>;
    fn send(&self, msg: Message, addr: &str) -> Result<()>;
}

pub use udp_transport::UdpTransport;
