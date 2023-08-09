use anyhow::{bail, Result};
use clap::{arg, Parser};
use log::info;
use messages::{ApplicationAPI, Message};
use transports::{Transport, UdpTransport, MAX_MTU};

#[derive(Parser, Debug, Clone)]
#[clap(version, long_about = None, propagate_version = true)]
#[clap(about = "Control a Myceli instance")]
pub struct Cli {
    #[arg(help = "The network address that a myceli instance is listening on")]
    instance_addr: String,
    #[arg(
        short,
        long,
        default_value = "512",
        help = "The MTU (in bytes) that messages are chunked into."
    )]
    mtu: u16,
    #[arg(
        short,
        long,
        help = "An optional delay (in milliseconds) between sending chunks."
    )]
    chunk_transmit_throttle: Option<u32>,
    #[arg(short, long, help = "Listens for a response from the myceli instance")]
    listen_mode: bool,
    #[arg(
        short,
        long,
        default_value = "0.0.0.0:8200",
        help = "An optional network address to bind to"
    )]
    bind_address: String,
    #[clap(subcommand)]
    command: ApplicationAPI,
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        let transport =
            UdpTransport::new(&self.bind_address, self.mtu, self.chunk_transmit_throttle)?;

        let command = Message::ApplicationAPI(self.command.clone());
        let cmd_str = serde_json::to_string(&command)?;
        info!("Transmitting: {}", &cmd_str);

        transport.send(command, &self.instance_addr)?;
        if self.listen_mode {
            match transport.receive() {
                Ok((msg, _)) => {
                    info!("Received: {msg:?}");
                    println!("Received: {msg:?}");
                    return Ok(());
                }
                Err(e) => bail!("{e:?}"),
            }
        }

        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    if cli.mtu > MAX_MTU {
        bail!("Configured MTU is too large, cannot exceed {MAX_MTU}",);
    }

    cli.run().await
}
