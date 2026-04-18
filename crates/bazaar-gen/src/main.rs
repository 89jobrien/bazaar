mod config;
mod error;
mod fetch;
mod header;
mod model;
mod port;
mod render;

use chrono::Utc;
use clap::Parser;
use config::Config;
use fetch::{
    crates_io::CratesIoFetcher,
    github::GitHubFetcher,
    plugins::PluginFetcher,
    pypi::PypiFetcher,
};
use header::HeaderConfig;
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
    #[arg(long, default_value = "examples/header.yaml")]
    header_config: PathBuf,
    #[arg(long, default_value = "examples/showcase.yaml")]
    showcase_yaml: PathBuf,
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

    let merged = model::merge(all);
    eprintln!("{} projects after merge", merged.len());

    // Load header config and apply overrides/pinned/tags
    let hcfg = HeaderConfig::load(&args.header_config)?;
    let projects = hcfg.apply(merged);

    let html = render::html::render_html(
        &config.github_user,
        &config.crates_io_user,
        &hcfg.title,
        &hcfg.subtitle,
        &projects,
    )?;
    std::fs::write(&args.output, &html)?;
    eprintln!("wrote {}", args.output.display());

    let md = render::markdown::render_readme(&projects, &hcfg.title, &hcfg.subtitle)?;
    std::fs::write(&args.readme, &md)?;
    eprintln!("wrote {}", args.readme.display());

    // Write showcase.yaml
    #[derive(serde::Serialize)]
    struct ShowcaseYaml<'a> {
        generated: String,
        projects: &'a [model::Project],
    }
    let showcase = ShowcaseYaml {
        generated: Utc::now().to_rfc3339(),
        projects: &projects,
    };
    let yaml_text = serde_yaml::to_string(&showcase)?;
    std::fs::write(&args.showcase_yaml, yaml_text)?;
    eprintln!("wrote {}", args.showcase_yaml.display());

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
