use anyhow::Result;

use serde_json::from_slice;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::info;

use crate::api::ApplicationAPI;

pub async fn control(listen_addr: &String) -> Result<()> {
    info!("Listening for control messages on {}", listen_addr);
    let listen_address: SocketAddr = listen_addr.parse()?;
    let socket = UdpSocket::bind(&listen_address).await?;

    let mut buf = vec![0; 10240];

    let val = ApplicationAPI::ImportFile("/dev/null".to_owned());
    info!("Try this: {:?}", serde_json::to_string(&val));

    loop {
        if let Ok(len) = socket.try_recv(&mut buf) {
            if len > 0 {
                if let Ok(msg) = from_slice::<ApplicationAPI>(&buf[..len]) {
                    match msg {
                        ApplicationAPI::ImportFile(file) => {
                            info!("Received ImportFile: {}", file)
                        }
                        ApplicationAPI::ExportCid(cid) => {
                            info!("Received ExportCid: {}", cid)
                        }
                        ApplicationAPI::IsConnected(status) => {
                            info!("Received IsConnected: {}", status)
                        }
                        ApplicationAPI::IsCidComplete(cid) => {
                            info!("Received ISCidComplete: {}", cid)
                        }
                    }
                }
            }
        }
        sleep(Duration::from_millis(10));
    }
}
