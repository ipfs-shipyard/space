use anyhow::Result;
use clap::Parser;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = car_utility::run::Cli::parse();
    cli.run().await
}
