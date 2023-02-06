use anyhow::Result;
use clap::{arg, Parser};
use messages::{ApplicationAPI, Message};
use parity_scale_codec::Decode;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[derive(Parser, Debug, Clone)]
#[clap(version, long_about = None, propagate_version = true)]
#[clap(about = "Control an IPFS instance")]
pub struct Cli {
    instance_addr: String,
    #[arg(short, long)]
    listen: bool,
    #[clap(subcommand)]
    command: ApplicationAPI,
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        let command = Message::ApplicationAPI(self.command.clone());
        let cmd_str = serde_json::to_string(&command)?;
        println!("Transmitting: {}", &cmd_str);
        let target_address: SocketAddr = self.instance_addr.parse()?;
        let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
        let socket = UdpSocket::bind(&bind_address).await?;
        socket.send_to(&command.to_bytes(), target_address).await?;
        if self.listen {
            let mut buf = vec![0; 1024];
            if let Ok(len) = socket.recv(&mut buf).await {
                let mut databuf = &buf[..len];
                match Message::decode(&mut databuf) {
                    Ok(msg) => println!("{msg:?}"),
                    Err(e) => println!("{e:?}"),
                }
            }
        }
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await
}
