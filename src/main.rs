mod auth;
mod cli;
mod error;
mod insights;
mod providers;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    info!("Starting CILens - CI/CD Insights Tool");
    cli.execute().await?;

    Ok(())
}
