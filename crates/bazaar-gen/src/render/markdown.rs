use crate::model::{Kind, Project};
use anyhow::Result;
use chrono::Utc;

fn kind_label(kinds: &[Kind]) -> String {
    kinds
        .iter()
        .map(|k| match k {
            Kind::GitHubRepo => "Repo",
            Kind::CratesIo => "Crate",
            Kind::PyPI => "PyPI",
            Kind::ClaudePlugin => "Plugin",
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

pub fn render_readme(projects: &[Project], title: &str, subtitle: &str) -> Result<String> {
    let mut out = format!("# {title}\n\n{subtitle}\n\n");
    out.push_str(&format!(
        "\n_Generated {}_\n\n",
        Utc::now().format("%Y-%m-%d")
    ));
    out.push_str("| Project | Kind | Tags | Description | Updated |\n");
    out.push_str("|---|---|---|---|---|\n");

    for p in projects {
        let name = format!("[{}]({})", p.name, p.url);
        let kind = kind_label(&p.kinds);
        let tags = if p.tags.is_empty() {
            "—".to_string()
        } else {
            p.tags.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(" ")
        };
        let desc = p.description.as_deref().unwrap_or("—");
        let updated = p
            .pushed_at
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "—".to_string());
        out.push_str(&format!("| {name} | {kind} | {tags} | {desc} | {updated} |\n"));
    }

    Ok(out)
}
