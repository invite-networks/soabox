//! SOAbox command line entry point.

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    soabox::cli::run(soabox::cli::Cli::parse()).await
}
