use anyhow::Result;
use parity_scale_codec::Decode;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{error, info};

use crate::receive::receive;
use crate::receiver::Receiver;
use crate::transmit::transmit;
use messages::{ApplicationAPI, Message};

// TODO: Make this configurable
pub const MTU: usize = 60;

pub async fn control(listen_addr: &String) -> Result<()> {
    info!("Listening for messages on {}", listen_addr);
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
            Ok(Message::ApplicationAPI(ApplicationAPI::Receive { listen_addr })) => {
                receive(&listen_addr).await?
            }
            Ok(Message::ApplicationAPI(ApplicationAPI::Transmit { path, target_addr })) => {
                transmit(&PathBuf::from(path), &target_addr).await?
            }
            Ok(Message::DataProtocol(data_msg)) => {
                data_receiver.handle_transmission_msg(data_msg).await?;
                data_receiver.attempt_tree_assembly().await?;
            }
            Ok(message) => {
                info!("Received unhandled message: {:?}", message);
            }
            Err(err) => {
                error!("{err}");
            }
        }
    }
}
