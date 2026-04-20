use crate::model::{Kind, Project};
use anyhow::Result;
use askama::Template;
use chrono::Utc;

#[derive(Clone)]
struct CommitDisplay {
    message: String,
}

#[derive(Clone)]
struct ProjectDisplay {
    slug: String,
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
        slug: p.slug(),
        name: p.name.clone(),
        description: p.description.clone(),
        url: p.url.clone(),
        kinds: p.kinds.clone(),
        kinds_str,
        language: p.language.clone(),
        pushed_at: p.pushed_at.as_ref().map(|dt| dt.format("%Y-%m-%d").to_string()),
        version: p.version.clone(),
        stars: p.stars,
        downloads: p.downloads,
        recent_commits: p.recent_commits.iter().take(max_commits).map(|c| CommitDisplay {
            message: c.message.clone(),
        }).collect(),
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
    data_json: String,
}

#[derive(Template)]
#[template(path = "project.html")]
struct ProjectTemplate {
    project: ProjectDisplay,
    generated_at: String,
}

pub fn render_html(
    username: &str,
    crates_user: &str,
    title: &str,
    subtitle: &str,
    projects: &[Project],
    data_json: &str,
    max_commits: usize,
) -> Result<String> {
    let display_projects = projects.iter().map(|p| project_display(p, max_commits)).collect::<Vec<_>>();
    let projects_count = display_projects.len();
    let tmpl = IndexTemplate {
        username,
        crates_user,
        title,
        subtitle,
        projects: display_projects,
        projects_count,
        generated_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
        data_json: data_json.to_string(),
    };
    tmpl.render().map_err(|e| anyhow::anyhow!("template render failed: {e}"))
}

pub fn render_project_html(_username: &str, project: &Project, max_commits: usize) -> Result<String> {
    let tmpl = ProjectTemplate {
        project: project_display(project, max_commits),
        generated_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
    };
    tmpl.render().map_err(|e| anyhow::anyhow!("project template render failed: {e}"))
}
