use anyhow::Result;
use clap::Parser;
use messages::{ApplicationAPI, Message, MessageChunker, SimpleChunker, UnchunkResult};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;

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

        let mut tries: i32 = 0;
        while tries < 50 {
            let mut buf = vec![0; 1024];
            if socket.try_recv(&mut buf).is_ok() {
                println!("control recvd");
                match chunker.unchunk(&buf) {
                    Ok(Some(UnchunkResult::Message(msg))) => {
                        println!("{msg:?}");
                        // return Ok(());
                    }
                    Ok(Some(UnchunkResult::Missing(m))) => {
                        println!("why did we get a missing msg? {m:?}");
                    }
                    // No assembly errors and nothing assembled yet
                    Ok(None) => {
                        continue;
                    }
                    Err(e) => println!("{e:?}"),
                }
            }

            if tries.rem_euclid(10) == 0 {
                let missing_chunks = chunker.find_missing_chunks()?;

                for msg in missing_chunks {
                    socket.send_to(&msg, target_address).await?;
                }
            }
            tries += 1;
            sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await
}
