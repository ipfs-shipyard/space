use anyhow::Result;
use block_streamer::api::ApplicationAPI;
use clap::Parser;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[derive(Parser, Debug, Clone)]
#[clap(version, long_about = None, propagate_version = true)]
#[clap(about = "Control an IPFS instance")]
pub struct Cli {
    instance_addr: String,
    #[clap(subcommand)]
    command: ApplicationAPI,
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        let target_address: SocketAddr = self.instance_addr.parse()?;
        let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
        let socket = UdpSocket::bind(&bind_address).await?;
        let cmd_str = serde_json::to_string(&self.command)?;
        println!("Transmitting: {}", &cmd_str);
        socket.send_to(cmd_str.as_bytes(), target_address).await?;
        Ok(())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await
}
