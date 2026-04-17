use crate::model::{Kind, Project};
use anyhow::Result;
use askama::Template;
use chrono::Utc;

#[derive(Clone)]
struct ProjectDisplay {
    name: String,
    description: Option<String>,
    url: String,
    kinds: Vec<Kind>,
    language: Option<String>,
    pushed_at: Option<String>,
    version: Option<String>,
    stars: Option<u32>,
    downloads: Option<u64>,
}

fn project_display(p: &Project) -> ProjectDisplay {
    ProjectDisplay {
        name: p.name.clone(),
        description: p.description.clone(),
        url: p.url.clone(),
        kinds: p.kinds.clone(),
        language: p.language.clone(),
        pushed_at: p.pushed_at.as_ref().map(|dt| dt.format("%Y-%m-%d").to_string()),
        version: p.version.clone(),
        stars: p.stars,
        downloads: p.downloads,
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    username: &'a str,
    crates_user: &'a str,
    projects: Vec<ProjectDisplay>,
    projects_count: usize,
    generated_at: String,
}

pub fn render_html(username: &str, crates_user: &str, projects: &[Project]) -> Result<String> {
    let display_projects = projects.iter().map(project_display).collect::<Vec<_>>();
    let projects_count = display_projects.len();
    let tmpl = IndexTemplate {
        username,
        crates_user,
        projects: display_projects,
        projects_count,
        generated_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
    };
    tmpl.render().map_err(|e| anyhow::anyhow!("template render failed: {e}"))
}
