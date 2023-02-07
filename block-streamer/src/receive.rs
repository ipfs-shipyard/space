use crate::control::MTU;
use crate::receiver::Receiver;
use anyhow::Result;
use messages::Message;
use parity_scale_codec::Decode;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{error, info, warn};

pub async fn receive(listen_addr: &String) -> Result<()> {
    info!("Listening for blocks at {}", listen_addr);
    let listen_address: SocketAddr = listen_addr.parse()?;

    let mut buf = vec![0; MTU];
    let mut real_len;
    let mut data_receiver = Receiver::new();

    let socket = UdpSocket::bind(&listen_address).await?;
    loop {
        {
            loop {
                if let Ok(len) = socket.try_recv(&mut buf) {
                    if len > 0 {
                        real_len = len;
                        break;
                    }
                }
                sleep(Duration::from_millis(10));
            }
        }

        let mut databuf = &buf[..real_len];
        match Message::decode(&mut databuf) {
            Ok(Message::DataProtocol(msg)) => {
                data_receiver.handle_transmission_msg(msg).await?;
                data_receiver.attempt_tree_assembly().await?;
            }
            Ok(other) => {
                warn!("Received API message: {:?}", other)
            }
            Err(err) => {
                error!("Decode failed: {err}");
            }
        }
    }
}
