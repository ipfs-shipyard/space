mod handlers;
mod receiver;
mod server;
mod transmit;

use anyhow::Result;
use clap::Parser;
use server::Server;
use tracing::Level;

#[derive(Parser, Debug)]
#[clap(about = "Spacey IPFS Node")]
struct Args {
    listen_address: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();
    let mut server = Server::new(&args.listen_address)
        .await
        .expect("Server creation failed");
    server
        .listen()
        .await
        .expect("Error encountered in server operation");
    Ok(())
}
