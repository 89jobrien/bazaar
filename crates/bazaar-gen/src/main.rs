mod config;
mod error;
mod model;
mod port;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "bz", about = "bazaar showcase generator")]
struct Args {
    #[arg(long, default_value = "index.html")]
    output: PathBuf,
    #[arg(long, default_value = "README.md")]
    readme: PathBuf,
    #[arg(long, default_value = "pypi.toml")]
    pypi_toml: PathBuf,
    #[arg(long, default_value = ".claude-plugin/marketplace.json")]
    plugin_manifest: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = config::Config::from_env(&args.pypi_toml, args.plugin_manifest)?;
    println!("config loaded for user: {}", config.github_user);
    Ok(())
}
