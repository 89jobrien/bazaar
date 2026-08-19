mod config;
mod deploy;
mod enrich;
mod fetch;
mod header;
mod model;
mod port;
mod render;

use chrono::Utc;
use clap::Parser;
use config::Config;
use enrich::CruxPipelineRunner;
use fetch::{
    crates_io::CratesIoFetcher, github::GitHubFetcher, insights::load_insights,
    plugins::PluginFetcher, profile::load_profile, pypi::PypiFetcher, usage::load_usage,
};
use futures::future::try_join_all;
use header::HeaderConfig;
use model::Project;
use port::{PipelineRunner, SourceFetcher};
use reqwest::Client;
use std::path::PathBuf;
use std::time::Duration;

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

/// Composition root helper: build the concrete source-fetcher adapters for
/// this run. Callers depend only on `SourceFetcher`, not these concrete types.
fn build_fetchers(client: &Client, args: &Args, config: &Config) -> Vec<Box<dyn SourceFetcher>> {
    vec![
        Box::new(GitHubFetcher {
            client: client.clone(),
            user: config.github_user.clone(),
            token: config.github_token.clone(),
        }),
        Box::new(CratesIoFetcher {
            client: client.clone(),
            user: config.crates_io_user.clone(),
        }),
        Box::new(PypiFetcher {
            client: client.clone(),
            packages: config.pypi_packages.clone(),
        }),
        Box::new(PluginFetcher {
            manifest_path: args.plugin_manifest.clone(),
        }),
    ]
}

/// Fetch and flatten projects from every injected source, concurrently.
async fn fetch_all(fetchers: &[Box<dyn SourceFetcher>]) -> anyhow::Result<Vec<Project>> {
    let batches = try_join_all(fetchers.iter().map(|f| f.fetch())).await?;
    Ok(batches.into_iter().flatten().collect())
}

/// Merge fetched projects, apply header overrides, and optionally enrich.
fn build_projects(
    fetched: Vec<Project>,
    header_config: &std::path::Path,
    runner: &dyn PipelineRunner,
    enrich_enabled: bool,
    force_enrich: bool,
) -> anyhow::Result<(Vec<Project>, HeaderConfig)> {
    let merged = model::merge(fetched);
    eprintln!("{} projects after merge", merged.len());

    let hcfg = HeaderConfig::load(header_config)?;
    let mut projects = hcfg.apply(merged);

    if enrich_enabled {
        let pipeline_dir = PathBuf::from("examples");
        let cache_path = PathBuf::from(".ctx/enrich-cache.json");
        enrich::enrich(
            runner,
            &mut projects,
            &pipeline_dir,
            &cache_path,
            force_enrich,
        )?;
        eprintln!("enrichment complete");
    }

    Ok((projects, hcfg))
}

/// Apply insights overrides (if any) onto a loaded profile.
fn apply_insights(profile: &mut model::Profile, insights: Option<&model::Insights>) {
    let Some(ins) = insights else { return };

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

/// Write every generated artifact (data files, HTML, README, showcase) to `out`.
#[allow(clippy::too_many_arguments)]
fn write_outputs(
    out: &std::path::Path,
    args: &Args,
    config: &Config,
    hcfg: &HeaderConfig,
    projects: &[Project],
    profile: &model::Profile,
    usage: Option<&model::UsageSnapshot>,
) -> anyhow::Result<()> {
    let data_dir = out.join("data");
    std::fs::create_dir_all(&data_dir)?;

    let data_yaml = render::yaml::render_data_yaml(projects)?;
    std::fs::write(out.join("data.yaml"), &data_yaml)?;
    eprintln!("wrote data.yaml");

    let profile_yaml = std::fs::read_to_string(&args.profile)?;
    std::fs::write(data_dir.join("profile.yaml"), &profile_yaml)?;
    eprintln!("wrote data/profile.yaml");

    if args.export_json {
        let data_json = render::json::render_data_json(projects)?;
        std::fs::write(out.join("data.json"), &data_json)?;
        eprintln!("wrote data.json");
    }

    let html = render::html::render_html(
        &config.github_user,
        &config.crates_io_user,
        &hcfg.title,
        &hcfg.subtitle,
        projects,
        profile,
        &data_yaml,
        args.max_commits,
    )?;
    std::fs::write(out.join("index.html"), &html)?;
    eprintln!("wrote index.html");

    let profile_dir = out.join("profile");
    std::fs::create_dir_all(&profile_dir)?;
    let profile_html = render::html::render_profile_html(profile, usage)?;
    std::fs::write(profile_dir.join("index.html"), profile_html)?;
    eprintln!("wrote profile/index.html");

    let md = render::markdown::render_readme(projects, &hcfg.title, &hcfg.subtitle)?;
    std::fs::write(&args.readme, &md)?;
    eprintln!("wrote {}", args.readme.display());

    #[derive(serde::Serialize)]
    struct ShowcaseYaml<'a> {
        generated: String,
        projects: &'a [Project],
    }
    let showcase = ShowcaseYaml {
        generated: Utc::now().to_rfc3339(),
        projects,
    };
    std::fs::write(&args.showcase_yaml, serde_yaml::to_string(&showcase)?)?;
    eprintln!("wrote {}", args.showcase_yaml.display());

    Ok(())
}

async fn generate(
    client: &Client,
    args: &Args,
    config: &Config,
    output_dir: &PathBuf,
    insights: Option<&model::Insights>,
) -> anyhow::Result<()> {
    let fetchers = build_fetchers(client, args, config);

    let mut profile = load_profile(&args.profile)?;
    eprintln!("loaded profile: {}", profile.name);
    apply_insights(&mut profile, insights);

    let usage = load_usage(&args.usage)?;
    if let Some(ref u) = usage {
        eprintln!(
            "loaded usage: ${:.2} total across {} days",
            u.totals.total_cost,
            u.daily.len()
        );
    }

    eprintln!("fetching from all sources...");
    let fetched = fetch_all(&fetchers).await?;

    let runner = CruxPipelineRunner;
    let (projects, hcfg) = build_projects(
        fetched,
        &args.header_config,
        &runner,
        args.enrich,
        args.force_enrich,
    )?;

    // When deploying, generate into a tempdir; otherwise use output_dir
    let tmp;
    let out: &std::path::Path = if args.deploy {
        tmp = tempfile::tempdir()?;
        tmp.path()
    } else {
        std::fs::create_dir_all(output_dir)?;
        output_dir.as_path()
    };

    write_outputs(
        out,
        args,
        config,
        &hcfg,
        &projects,
        &profile,
        usage.as_ref(),
    )?;

    if args.deploy {
        deploy::deploy(out, &args.deploy_repo, config.github_token.as_deref())?;
    }

    Ok(())
}

fn resolve_output_dir(args: &Args) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    args.output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{home}/dev/89jobrien.github.io")))
}

async fn run_watch_loop(
    client: &Client,
    args: &Args,
    config: &Config,
    output_dir: &PathBuf,
    insights: Option<&model::Insights>,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(args.interval));
    loop {
        ticker.tick().await;
        let ts = chrono::Local::now().format("%H:%M:%S");
        eprintln!("[{ts}] fetching...");
        match generate(client, args, config, output_dir, insights).await {
            Ok(()) => eprintln!("[{ts}] done — next in {}s", args.interval),
            Err(e) => eprintln!("[{ts}] error (continuing): {e:#}"),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::from_env(&args.pypi_toml)?;
    let client = Client::new();
    let output_dir = resolve_output_dir(&args);

    let insights = load_insights(&args.insights)?;
    if let Some(ref ins) = insights {
        eprintln!("loaded insights: {:?}", ins.generated_at);
    }

    if args.watch {
        run_watch_loop(&client, &args, &config, &output_dir, insights.as_ref()).await;
        Ok(())
    } else {
        generate(&client, &args, &config, &output_dir, insights.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    /// Test double for `SourceFetcher` — no network, no filesystem.
    struct MockFetcher {
        projects: Vec<Project>,
    }

    #[async_trait::async_trait]
    impl SourceFetcher for MockFetcher {
        async fn fetch(&self) -> anyhow::Result<Vec<Project>> {
            Ok(self.projects.clone())
        }
    }

    fn stub_project(name: &str) -> Project {
        Project {
            name: name.to_string(),
            description: None,
            url: String::new(),
            kinds: vec![Kind::GitHubRepo],
            language: None,
            pushed_at: None,
            version: None,
            stars: None,
            downloads: None,
            recent_commits: vec![],
            tags: vec![],
            topics: vec![],
            readme: None,
            category: None,
            changelog: None,
            health: None,
            related: vec![],
        }
    }

    struct NullPipelineRunner;
    impl PipelineRunner for NullPipelineRunner {
        fn run(
            &self,
            _pipeline: &std::path::Path,
            _input: &str,
        ) -> anyhow::Result<serde_json::Value> {
            anyhow::bail!(
                "NullPipelineRunner should never be called (enrich disabled in this test)"
            )
        }
    }

    #[tokio::test]
    async fn fetch_all_flattens_every_injected_source() {
        let fetchers: Vec<Box<dyn SourceFetcher>> = vec![
            Box::new(MockFetcher {
                projects: vec![stub_project("alpha")],
            }),
            Box::new(MockFetcher {
                projects: vec![stub_project("beta"), stub_project("gamma")],
            }),
        ];

        let fetched = fetch_all(&fetchers).await.unwrap();

        assert_eq!(fetched.len(), 3);
        let names: Vec<_> = fetched.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
        assert!(names.contains(&"gamma"));
    }

    #[tokio::test]
    async fn end_to_end_pipeline_runs_without_network() {
        let fetchers: Vec<Box<dyn SourceFetcher>> = vec![Box::new(MockFetcher {
            projects: vec![stub_project("solo-project")],
        })];
        let fetched = fetch_all(&fetchers).await.unwrap();

        let runner = NullPipelineRunner;
        let missing_header_config = std::path::Path::new("does-not-exist.yaml");
        let (projects, hcfg) =
            build_projects(fetched, missing_header_config, &runner, false, false).unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "solo-project");

        let yaml = render::yaml::render_data_yaml(&projects).unwrap();
        assert!(yaml.contains("solo-project"));
        assert!(hcfg.title.is_empty());
    }
}
