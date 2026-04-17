mod config;
mod error;
mod fetch;
mod model;
mod port;
mod publish;
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
    /// Push generated index.html to this GitHub repo (e.g. owner/owner.github.io).
    /// Overrides BAZAAR_PAGES_REPO env var.
    #[arg(long)]
    pages_repo: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::from_env(&args.pypi_toml, args.plugin_manifest.clone())?;
    let client = Client::new();

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
        manifest_path: args.plugin_manifest,
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

    let pages_repo = args.pages_repo.or(config.pages_repo);
    if let Some(repo) = pages_repo {
        let token = config.github_token.as_deref().unwrap_or_default();
        if token.is_empty() {
            eprintln!("warning: GITHUB_TOKEN not set — skipping pages push");
        } else {
            eprintln!("pushing index.html to {}...", repo);
            publish::push_to_pages(
                &client,
                token,
                &repo,
                "index.html",
                html.as_bytes(),
                "chore: regenerate showcase",
            )
            .await?;
            eprintln!("pushed to {}", repo);
        }
    }

    Ok(())
}
