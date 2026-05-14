use crate::model::{Kind, Profile, Project, UsageSnapshot};
use anyhow::Result;
use askama::Template;
use chrono::Utc;

#[derive(Clone)]
struct CommitDisplay {
    message: String,
}

#[derive(Clone)]
struct ProjectDisplay {
    name: String,
    description: Option<String>,
    url: String,
    kinds: Vec<Kind>,
    kinds_str: String,
    language: Option<String>,
    pushed_at: Option<String>,
    version: Option<String>,
    stars: Option<u32>,
    downloads: Option<u64>,
    recent_commits: Vec<CommitDisplay>,
    tags: Vec<String>,
    tags_str: String,
}

fn kind_token(k: &Kind) -> &'static str {
    match k {
        Kind::GitHubRepo => "gh",
        Kind::CratesIo => "crate",
        Kind::PyPI => "pypi",
        Kind::ClaudePlugin => "plugin",
    }
}

fn project_display(p: &Project, max_commits: usize) -> ProjectDisplay {
    let kinds_str = p.kinds.iter().map(kind_token).collect::<Vec<_>>().join(" ");
    let tags_str = p.tags.join(" ");
    ProjectDisplay {
        name: p.name.clone(),
        description: p.description.clone(),
        url: p.url.clone(),
        kinds: p.kinds.clone(),
        kinds_str,
        language: p.language.clone(),
        pushed_at: p
            .pushed_at
            .as_ref()
            .map(|dt| dt.format("%Y-%m-%d").to_string()),
        version: p.version.clone(),
        stars: p.stars,
        downloads: p.downloads,
        recent_commits: p
            .recent_commits
            .iter()
            .take(max_commits)
            .map(|c| CommitDisplay {
                message: c.message.clone(),
            })
            .collect(),
        tags: p.tags.clone(),
        tags_str,
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    username: &'a str,
    crates_user: &'a str,
    title: &'a str,
    subtitle: &'a str,
    projects: Vec<ProjectDisplay>,
    projects_count: usize,
    generated_at: String,
    data_yaml: String,
    profile: Profile,
}

struct UsageDisplay {
    total_tokens: String,
    total_cost: String,
    peak_day: String,
}

fn usage_display(u: &UsageSnapshot) -> UsageDisplay {
    let peak = u
        .peak_day()
        .map(|d| format!("{} — ${:.2}", d.date, d.total_cost))
        .unwrap_or_default();
    UsageDisplay {
        total_tokens: format!("{}", u.totals.total_tokens),
        total_cost: format!("{:.2}", u.totals.total_cost),
        peak_day: peak,
    }
}

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileTemplate {
    profile: Profile,
    usage_display: Option<UsageDisplay>,
    generated_at: String,
}

#[allow(clippy::too_many_arguments)]
pub fn render_html(
    username: &str,
    crates_user: &str,
    title: &str,
    subtitle: &str,
    projects: &[Project],
    profile: &Profile,
    data_yaml: &str,
    max_commits: usize,
) -> Result<String> {
    let display_projects = projects
        .iter()
        .map(|p| project_display(p, max_commits))
        .collect::<Vec<_>>();
    let projects_count = display_projects.len();
    let tmpl = IndexTemplate {
        username,
        crates_user,
        title,
        subtitle,
        projects: display_projects,
        projects_count,
        generated_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
        data_yaml: data_yaml.to_string(),
        profile: profile.clone(),
    };
    tmpl.render()
        .map_err(|e| anyhow::anyhow!("template render failed: {e}"))
}

pub fn render_profile_html(profile: &Profile, usage: Option<&UsageSnapshot>) -> Result<String> {
    let tmpl = ProfileTemplate {
        profile: profile.clone(),
        usage_display: usage.map(usage_display),
        generated_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
    };
    tmpl.render()
        .map_err(|e| anyhow::anyhow!("profile template render failed: {e}"))
}
