mod config;
mod deploy;
mod enrich;
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
    crates_io::CratesIoFetcher, github::GitHubFetcher, insights::load_insights,
    plugins::PluginFetcher, profile::load_profile, pypi::PypiFetcher, usage::load_usage,
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
    /// Output directory for generated site files
    #[arg(long)]
    output_dir: Option<PathBuf>,
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
    #[arg(long, default_value = "examples/profile.yaml")]
    profile: PathBuf,
    #[arg(long, default_value = "examples/ccusage.json")]
    usage: PathBuf,
    /// Path to insights.yaml (generated daily from Claude /insights)
    #[arg(long, default_value = "examples/insights.yaml")]
    insights: PathBuf,
    /// Run LLM enrichment (descriptions, changelog, category, related) via crux pipelines
    #[arg(long)]
    enrich: bool,
    /// Force re-enrichment even if cached results exist
    #[arg(long)]
    force_enrich: bool,
    /// Also export data as data.json alongside data.yaml
    #[arg(long)]
    export_json: bool,
    /// Push generated site directly to the GitHub Pages repo
    #[arg(long)]
    deploy: bool,
    /// GitHub repo to deploy to (owner/name)
    #[arg(long, default_value = "89jobrien/89jobrien.github.io")]
    deploy_repo: String,
    /// Maximum number of recent commits to show per project in HTML output
    #[arg(long, default_value = "3")]
    max_commits: usize,
    #[arg(long)]
    watch: bool,
    #[arg(long, default_value = "300")]
    interval: u64,
}

async fn generate(
    client: &Client,
    args: &Args,
    config: &Config,
    output_dir: &PathBuf,
    insights: Option<&model::Insights>,
) -> anyhow::Result<()> {
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

    let mut profile = load_profile(&args.profile)?;
    eprintln!("loaded profile: {}", profile.name);

    // Apply insights overrides to profile
    if let Some(ins) = insights {
        if let Some(ref s) = ins.summary {
            profile.summary = s.clone();
        }
        if let Some(ref t) = ins.tagline {
            profile.tagline = t.clone();
        }
        if let Some(ref r) = ins.role {
            profile.role = r.clone();
        }
        if !ins.focus_areas.is_empty() {
            profile.focus_areas = ins.focus_areas.clone();
        }
        if !ins.active_projects.is_empty() {
            profile.active_projects = ins
                .active_projects
                .iter()
                .map(|p| model::ProfileProject {
                    name: p.name.clone(),
                    description: p.description.clone(),
                    url: p.url.clone().unwrap_or_default(),
                })
                .collect();
        }
        if let Some(ref wf) = ins.workflow_style {
            profile.workflow_style = wf.clone();
        }
        if let Some(ref s) = ins.stats {
            if let Some(ref spd) = s.sessions_per_day {
                profile.stats.sessions_per_day = spd.clone();
            }
            if let Some(ts) = s.total_sessions {
                profile.stats.total_sessions_march_april_2026 = ts;
            }
            if let Some(ref stb) = s.spec_to_ship_best {
                profile.stats.spec_to_ship_best = stb.clone();
            }
        }
    }

    let usage = load_usage(&args.usage)?;
    if let Some(ref u) = usage {
        eprintln!(
            "loaded usage: ${:.2} total across {} days",
            u.totals.total_cost,
            u.daily.len()
        );
    }

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

    let hcfg = HeaderConfig::load(&args.header_config)?;
    let mut projects = hcfg.apply(merged);

    if args.enrich {
        let pipeline_dir = PathBuf::from("examples");
        let cache_path = PathBuf::from(".ctx/enrich-cache.json");
        enrich::enrich(&mut projects, &pipeline_dir, &cache_path, args.force_enrich)?;
        eprintln!("enrichment complete");
    }

    let projects = projects;

    // When deploying, generate into a tempdir; otherwise use output_dir
    let tmp;
    let out: &std::path::Path = if args.deploy {
        tmp = tempfile::tempdir()?;
        tmp.path()
    } else {
        std::fs::create_dir_all(output_dir)?;
        output_dir.as_path()
    };

    // data/
    let data_dir = out.join("data");
    std::fs::create_dir_all(&data_dir)?;

    // data.yaml
    let data_yaml = render::yaml::render_data_yaml(&projects)?;
    std::fs::write(out.join("data.yaml"), &data_yaml)?;
    eprintln!("wrote data.yaml");

    // data/profile.yaml
    let profile_yaml = std::fs::read_to_string(&args.profile)?;
    std::fs::write(data_dir.join("profile.yaml"), &profile_yaml)?;
    eprintln!("wrote data/profile.yaml");

    // data.json (optional)
    if args.export_json {
        let data_json = render::json::render_data_json(&projects)?;
        std::fs::write(out.join("data.json"), &data_json)?;
        eprintln!("wrote data.json");
    }

    // index.html
    let html = render::html::render_html(
        &config.github_user,
        &config.crates_io_user,
        &hcfg.title,
        &hcfg.subtitle,
        &projects,
        &profile,
        &data_yaml,
        args.max_commits,
    )?;
    std::fs::write(out.join("index.html"), &html)?;
    eprintln!("wrote index.html");

    // profile/index.html
    let profile_dir = out.join("profile");
    std::fs::create_dir_all(&profile_dir)?;
    let profile_html = render::html::render_profile_html(&profile, usage.as_ref())?;
    std::fs::write(profile_dir.join("index.html"), profile_html)?;
    eprintln!("wrote profile/index.html");

    // README + showcase.yaml always go to their configured paths
    let md = render::markdown::render_readme(&projects, &hcfg.title, &hcfg.subtitle)?;
    std::fs::write(&args.readme, &md)?;
    eprintln!("wrote {}", args.readme.display());

    #[derive(serde::Serialize)]
    struct ShowcaseYaml<'a> {
        generated: String,
        projects: &'a [model::Project],
    }
    let showcase = ShowcaseYaml {
        generated: Utc::now().to_rfc3339(),
        projects: &projects,
    };
    std::fs::write(&args.showcase_yaml, serde_yaml::to_string(&showcase)?)?;
    eprintln!("wrote {}", args.showcase_yaml.display());

    if args.deploy {
        deploy::deploy(out, &args.deploy_repo, config.github_token.as_deref())?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::from_env(&args.pypi_toml)?;
    let client = Client::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{home}/dev/89jobrien.github.io")));

    let insights = load_insights(&args.insights)?;
    if let Some(ref ins) = insights {
        eprintln!("loaded insights: {:?}", ins.generated_at);
    }

    if args.watch {
        let mut ticker = tokio::time::interval(Duration::from_secs(args.interval));
        loop {
            ticker.tick().await;
            let ts = chrono::Local::now().format("%H:%M:%S");
            eprintln!("[{ts}] fetching...");
            match generate(&client, &args, &config, &output_dir, insights.as_ref()).await {
                Ok(()) => eprintln!("[{ts}] done — next in {}s", args.interval),
                Err(e) => eprintln!("[{ts}] error (continuing): {e:#}"),
            }
        }
    } else {
        generate(&client, &args, &config, &output_dir, insights.as_ref()).await
    }
}
