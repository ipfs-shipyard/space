use anyhow::Result;
use clap::Parser;
use myceli::listener::Listener;
use tracing::Level;

#[derive(Parser, Debug)]
#[clap(about = "Myceli, a spacey IPFS node")]
struct Args {
    listen_address: String,
    #[arg(short, long, default_value_t = 100)]
    retry_timeout_duration: u32,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();

    let mut listener =
        Listener::new(&args.listen_address, "storage.db").expect("Listener creation failed");
    listener
        .start(args.retry_timeout_duration)
        .expect("Error encountered in listener operation");
    Ok(())
}
