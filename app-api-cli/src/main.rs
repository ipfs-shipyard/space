use anyhow::Result;
use clap::{arg, Parser};
use messages::chunking::{MessageChunker, SimpleChunker};
use messages::{ApplicationAPI, Message};
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
        let mut chunker = SimpleChunker::new(60);
        let command = Message::ApplicationAPI(self.command.clone());
        let cmd_str = serde_json::to_string(&command)?;
        println!("Transmitting: {}", &cmd_str);
        let target_address: SocketAddr = self.instance_addr.parse()?;
        let bind_address: SocketAddr = "127.0.0.1:0".parse()?;
        let socket = UdpSocket::bind(&bind_address).await?;
        for chunks in chunker.chunk(command).unwrap() {
            socket.send_to(&chunks, target_address).await?;
        }

        if self.listen {
            let mut buf = vec![0; 1024];
            if socket.recv(&mut buf).await.is_ok() {
                match chunker.unchunk(&buf) {
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
