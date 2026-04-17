mod config;
mod error;
mod fetch;
mod model;
mod port;
mod render;

use clap::Parser;
use config::Config;
use fetch::{
    crates_io::CratesIoFetcher,
    github::GitHubFetcher,
    plugins::PluginFetcher,
    pypi::PypiFetcher,
};
use port::SourceFetcher;
use reqwest::Client;
use std::path::PathBuf;
use std::time::Duration;
use tokio::try_join;

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
    #[arg(long, default_value = "README.header.md")]
    readme_header: PathBuf,
    #[arg(long)]
    watch: bool,
    #[arg(long, default_value = "300")]
    interval: u64,
}

async fn generate(client: &Client, args: &Args, config: &Config) -> anyhow::Result<()> {
    let github = GitHubFetcher {
        client: client.clone(),
        user: config.github_user.clone(),
        token: config.github_token.clone(),
    };
    let crates = CratesIoFetcher {
        client: client.clone(),
        user: config.crates_io_user.clone(),
    };
    let pypi = PypiFetcher {
        client: client.clone(),
        packages: config.pypi_packages.clone(),
    };
    let plugins = PluginFetcher {
        manifest_path: args.plugin_manifest.clone(),
    };

    eprintln!("fetching from all sources...");
    let (gh, cr, py, pl) = try_join!(
        github.fetch(),
        crates.fetch(),
        pypi.fetch(),
        plugins.fetch(),
    )?;

    let mut all = gh;
    all.extend(cr);
    all.extend(py);
    all.extend(pl);

    let projects = model::merge(all);
    eprintln!("{} projects after merge", projects.len());

    let html = render::html::render_html(&config.github_user, &config.crates_io_user, &projects)?;
    std::fs::write(&args.output, &html)?;
    eprintln!("wrote {}", args.output.display());

    let md = render::markdown::render_readme(&projects, &args.readme_header)?;
    std::fs::write(&args.readme, &md)?;
    eprintln!("wrote {}", args.readme.display());

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::from_env(&args.pypi_toml)?;
    let client = Client::new();

    if args.watch {
        let mut ticker = tokio::time::interval(Duration::from_secs(args.interval));
        loop {
            ticker.tick().await;
            let ts = chrono::Local::now().format("%H:%M:%S");
            eprintln!("[{ts}] fetching...");
            match generate(&client, &args, &config).await {
                Ok(()) => eprintln!("[{ts}] done — next in {}s", args.interval),
                Err(e) => eprintln!("[{ts}] error (continuing): {e:#}"),
            }
        }
    } else {
        generate(&client, &args, &config).await
    }
}
