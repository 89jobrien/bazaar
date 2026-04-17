use crate::model::{Kind, Project};
use anyhow::Result;
use chrono::Utc;
use std::path::Path;

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

pub fn render_readme(projects: &[Project], header_path: &Path) -> Result<String> {
    let header = if header_path.exists() {
        std::fs::read_to_string(header_path)?
    } else {
        String::from("# Open Source\n\nProjects I maintain or contribute to.\n\n")
    };

    let mut out = header;
    out.push_str(&format!(
        "\n_Generated {}_\n\n",
        Utc::now().format("%Y-%m-%d")
    ));
    out.push_str("| Project | Kind | Description | Updated |\n");
    out.push_str("|---|---|---|---|\n");

    for p in projects {
        let name = format!("[{}]({})", p.name, p.url);
        let kind = kind_label(&p.kinds);
        let desc = p.description.as_deref().unwrap_or("—");
        let updated = p
            .pushed_at
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "—".to_string());
        out.push_str(&format!("| {name} | {kind} | {desc} | {updated} |\n"));
    }

    Ok(out)
}
