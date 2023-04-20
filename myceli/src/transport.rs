use anyhow::Result;
use messages::Message;

pub trait Transport: Send + Sync {
    fn new(listen_addr: &str, mtu: u16) -> Result<Self>
    where
        Self: Sized;
    fn receive(&self) -> Result<(Message, String)>;
    fn send(&self, msg: Message, addr: &str) -> Result<()>;
}
