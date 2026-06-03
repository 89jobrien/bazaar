use crate::model::{Project, ProjectStatus};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Cached enrichment results keyed by project slug.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EnrichCache(HashMap<String, EnrichEntry>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichEntry {
    pub description: Option<String>,
    pub category: Option<String>,
    pub changelog: Option<String>,
    pub health: Option<String>,
    pub related: Vec<String>,
}

impl EnrichCache {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn get(&self, slug: &str) -> Option<&EnrichEntry> {
        self.0.get(slug)
    }

    pub fn set(&mut self, slug: String, entry: EnrichEntry) {
        self.0.insert(slug, entry);
    }
}

fn crux_run(pipeline: &Path, input_json: &str) -> Result<serde_json::Value> {
    let input_file = tempfile::NamedTempFile::new()?;
    std::fs::write(input_file.path(), input_json)?;

    let output = Command::new("cruxx")
        .arg("run")
        .arg(pipeline)
        .arg(input_file.path())
        .output()
        .context("cruxx not found — is cruxx-agentic installed? (`cargo install --path crates/cruxx-agentic`)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cruxx run failed: {stderr}");
    }

    // crux-run emits structured output after "Output:" line
    let stdout = String::from_utf8_lossy(&output.stdout);
    let output_section = stdout
        .lines()
        .skip_while(|l| !l.starts_with("Output:"))
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");

    serde_json::from_str(output_section.trim()).context("failed to parse crux-run output as JSON")
}

fn needs_enrichment(project: &Project, force: bool) -> bool {
    force
        || project.description.is_none()
        || project.category.is_none()
        || project.changelog.is_none()
        || project.related.is_empty()
}

fn apply_status(project: &mut Project) {
    if project.health.is_none() {
        project.health = Some(
            ProjectStatus::from_pushed_at(project.pushed_at)
                .as_str()
                .to_string(),
        );
    }
}

fn build_entry(
    project: &Project,
    cached: Option<EnrichEntry>,
    all_names: &[String],
    pipeline_dir: &Path,
    force: bool,
) -> EnrichEntry {
    let mut entry = cached.unwrap_or(EnrichEntry {
        description: None,
        category: None,
        changelog: None,
        health: None,
        related: vec![],
    });

    if project.description.is_none() || force {
        if let Ok(out) = run_describe(project, pipeline_dir) {
            entry.description = Some(out);
        }
    }
    if project.category.is_none() || force {
        if let Ok(out) = run_classify(project, pipeline_dir) {
            entry.category = Some(out);
        }
    }
    if project.changelog.is_none() || force {
        if let Ok(out) = run_changelog(project, pipeline_dir) {
            entry.changelog = Some(out);
        }
    }
    if project.health.is_none() || force {
        if let Ok(out) = run_health(project, pipeline_dir) {
            entry.health = Some(out);
        }
    }
    if project.related.is_empty() || force {
        if let Ok(out) = run_related(project, all_names, pipeline_dir) {
            entry.related = out;
        }
    }

    entry
}

fn enrich_project(
    project: &mut Project,
    cache: &mut EnrichCache,
    all_names: &[String],
    pipeline_dir: &Path,
    force: bool,
) {
    let slug = project.slug();

    if let Some(entry) = cache.get(&slug) {
        apply_entry(project, entry.clone());
    }
    apply_status(project);

    if !needs_enrichment(project, force) {
        return;
    }

    eprintln!("enriching {}...", project.name);
    let cached = cache.get(&slug).cloned();
    let entry = build_entry(project, cached, all_names, pipeline_dir, force);
    apply_entry(project, entry.clone());
    cache.set(slug, entry);
}

pub fn enrich(
    projects: &mut [Project],
    pipeline_dir: &Path,
    cache_path: &Path,
    force: bool,
) -> Result<()> {
    let mut cache = EnrichCache::load(cache_path);
    let all_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();

    for project in projects.iter_mut() {
        enrich_project(project, &mut cache, &all_names, pipeline_dir, force);
    }

    cache.save(cache_path)?;
    Ok(())
}

fn apply_entry(project: &mut Project, entry: EnrichEntry) {
    if project.description.is_none() {
        project.description = entry.description;
    }
    if project.category.is_none() {
        project.category = entry.category;
    }
    if project.changelog.is_none() {
        project.changelog = entry.changelog;
    }
    if project.health.is_none() {
        project.health = entry.health;
    }
    if project.related.is_empty() {
        project.related = entry.related;
    }
}

fn commit_messages(project: &Project) -> Vec<&str> {
    project
        .recent_commits
        .iter()
        .map(|c| c.message.as_str())
        .collect()
}

fn run_pipeline(
    pipeline_dir: &Path,
    file: &str,
    payload: serde_json::Value,
    result_key: &str,
) -> Result<String> {
    let out = crux_run(&pipeline_dir.join(file), &payload.to_string())?;
    Ok(out[result_key].as_str().unwrap_or("").to_string())
}

fn run_describe(project: &Project, pipeline_dir: &Path) -> Result<String> {
    let commits = commit_messages(project);
    run_pipeline(
        pipeline_dir,
        "enrich_describe.crux",
        serde_json::json!({
            "function": "DescribeProject",
            "input": {
                "name": project.name,
                "language": project.language,
                "readme": project.readme,
                "commits": commits,
            }
        }),
        "description",
    )
}

fn run_classify(project: &Project, pipeline_dir: &Path) -> Result<String> {
    let commits = commit_messages(project);
    run_pipeline(
        pipeline_dir,
        "enrich_classify.crux",
        serde_json::json!({
            "function": "ClassifyProject",
            "input": {
                "name": project.name,
                "description": project.description,
                "language": project.language,
                "topics": project.topics,
                "commits": commits,
            }
        }),
        "category",
    )
}

fn run_changelog(project: &Project, pipeline_dir: &Path) -> Result<String> {
    let commits = commit_messages(project);
    if commits.is_empty() {
        return Ok(String::new());
    }
    let input = serde_json::json!({
        "function": "GenerateChangelog",
        "input": {
            "name": project.name,
            "commits": commits,
        }
    });
    let out = crux_run(
        &pipeline_dir.join("enrich_changelog.crux"),
        &input.to_string(),
    )?;
    Ok(out["summary"].as_str().unwrap_or("").to_string())
}

fn run_health(project: &Project, pipeline_dir: &Path) -> Result<String> {
    let commit_dates: Vec<String> = project
        .recent_commits
        .iter()
        .map(|c| c.date.format("%Y-%m-%d").to_string())
        .collect();
    let input = serde_json::json!({
        "function": "AssessHealth",
        "input": {
            "name": project.name,
            "pushed_at": project.pushed_at.map(|d| d.format("%Y-%m-%d").to_string()),
            "commit_dates": commit_dates,
            "open_issues": null,
        }
    });
    let out = crux_run(&pipeline_dir.join("enrich_health.crux"), &input.to_string())?;
    Ok(out["status"].as_str().unwrap_or("").to_string())
}

fn run_related(
    project: &Project,
    all_names: &[String],
    pipeline_dir: &Path,
) -> Result<Vec<String>> {
    let input = serde_json::json!({
        "function": "SuggestRelated",
        "input": {
            "name": project.name,
            "description": project.description,
            "category": project.category,
            "all_projects": all_names,
        }
    });
    let out = crux_run(
        &pipeline_dir.join("enrich_related.crux"),
        &input.to_string(),
    )?;
    let related = out["related"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(related)
}
