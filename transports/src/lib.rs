mod chunking;
mod tcp_transport;
mod udp_chunking;
mod udp_transport;

use anyhow::Result;
use messages::Message;

pub trait Transport: Send + Sync {
    fn receive(&self) -> Result<(Message, String)>;
    fn send(&self, msg: Message, addr: &str) -> Result<()>;
}

pub use tcp_transport::TcpTransport;
pub use udp_transport::UdpTransport;
