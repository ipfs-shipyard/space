use anyhow::{bail, Result};
use clap::{arg, Parser};
use messages::{ApplicationAPI, Message};
use transports::{Transport, UdpTransport};

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
        let transport = UdpTransport::new(&self.bind_address, 512)?;

        let command = Message::ApplicationAPI(self.command.clone());
        let cmd_str = serde_json::to_string(&command)?;
        println!("Transmitting: {}", &cmd_str);

        transport.send(command, &self.instance_addr)?;
        if self.listen_mode {
            match transport.receive() {
                Ok((msg, _)) => {
                    println!("{msg:?}");
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
    let cli = Cli::parse();
    cli.run().await
}
