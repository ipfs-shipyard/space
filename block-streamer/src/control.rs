use anyhow::Result;

use serde_json::from_slice;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::info;

use crate::api::ApplicationAPI;
use crate::receive::receive;
use crate::transmit::transmit;

pub async fn control(listen_addr: &String) -> Result<()> {
    info!("Listening for control messages on {}", listen_addr);
    let listen_address: SocketAddr = listen_addr.parse()?;

    let mut buf = vec![0; 10240];
    let mut real_len;

    loop {
        {
            let socket = UdpSocket::bind(&listen_address).await?;
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

        if let Ok(msg) = from_slice::<ApplicationAPI>(&buf[..real_len]) {
            match msg {
                ApplicationAPI::Receive { path, listen_addr } => {
                    receive(&PathBuf::from(path), &listen_addr).await?
                }
                ApplicationAPI::Transmit { path, target_addr } => {
                    transmit(&PathBuf::from(path), &target_addr).await?
                }
                other => {
                    info!("Received {:?}", other);
                }
            }
        }
    }
}
