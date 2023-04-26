use anyhow::{anyhow, Result};
use clap::{arg, Parser};
use messages::{ApplicationAPI, Message, MessageChunker, SimpleChunker};
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;

#[derive(Parser, Debug, Clone)]
#[clap(version, long_about = None, propagate_version = true)]
#[clap(about = "Control a Myceli instance")]
pub struct Cli {
    instance_addr: String,
    #[arg(short, long)]
    listen_mode: bool,
    #[arg(short, long, default_value = "0.0.0.0:8090")]
    bind_address: String,
    #[clap(subcommand)]
    command: ApplicationAPI,
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        let mut chunker = SimpleChunker::new(60);
        let command = Message::ApplicationAPI(self.command.clone());
        let cmd_str = serde_json::to_string(&command)?;
        println!("Transmitting: {}", &cmd_str);
        let target_address = self
            .instance_addr
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Error parsing target address"))?;
        let bind_address: SocketAddr = self
            .bind_address
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Error parsing listen address"))?;
        let socket = UdpSocket::bind(&bind_address).await?;
        for chunks in chunker.chunk(command).unwrap() {
            socket.send_to(&chunks, target_address).await?;
        }

        if self.listen_mode {
            loop {
                let mut buf = vec![0; 1024];
                if socket.recv(&mut buf).await.is_ok() {
                    match chunker.unchunk(&buf) {
                        Ok(Some(msg)) => {
                            println!("{msg:?}");
                            return Ok(());
                        }
                        // No assembly errors and nothing assembled yet
                        Ok(None) => {}
                        Err(e) => println!("{e:?}"),
                    }
                }
                sleep(Duration::from_millis(10)).await;
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
