mod handlers;
mod listener;
mod receiver;
mod transmit;

use anyhow::Result;
use clap::Parser;
use listener::Listener;
use tracing::Level;

#[derive(Parser, Debug)]
#[clap(about = "Myceli, a spacey IPFS node")]
struct Args {
    listen_address: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();
    let mut listener = Listener::new(&args.listen_address, "storage.db")
        .await
        .expect("Listener creation failed");
    listener
        .listen()
        .await
        .expect("Error encountered in listener operation");
    Ok(())
}
