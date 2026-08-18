use crate::model::{Project, ProjectStatus};
use crate::port::PipelineRunner;
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

/// Adapter: runs enrichment pipelines via the external `crux` binary.
pub struct CruxPipelineRunner;

impl PipelineRunner for CruxPipelineRunner {
    fn run(&self, pipeline: &Path, input_json: &str) -> Result<serde_json::Value> {
        let input_file = tempfile::NamedTempFile::new()?;
        std::fs::write(input_file.path(), input_json)?;

        let output = Command::new("crux")
            .arg("run")
            .arg(pipeline)
            .arg(input_file.path())
            .output()
            .context("crux not found — is crux-agentic installed? (`cargo install --path crates/crux-agentic`)")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("crux run failed: {stderr}");
        }

        // crux-run emits structured output after "Output:" line
        let stdout = String::from_utf8_lossy(&output.stdout);
        let output_section = stdout
            .lines()
            .skip_while(|l| !l.starts_with("Output:"))
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");

        serde_json::from_str(output_section.trim())
            .context("failed to parse crux-run output as JSON")
    }
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
    runner: &dyn PipelineRunner,
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

    if (project.description.is_none() || force)
        && let Ok(out) = run_describe(runner, project, pipeline_dir)
    {
        entry.description = Some(out);
    }
    if (project.category.is_none() || force)
        && let Ok(out) = run_classify(runner, project, pipeline_dir)
    {
        entry.category = Some(out);
    }
    if (project.changelog.is_none() || force)
        && let Ok(out) = run_changelog(runner, project, pipeline_dir)
    {
        entry.changelog = Some(out);
    }
    if (project.health.is_none() || force)
        && let Ok(out) = run_health(runner, project, pipeline_dir)
    {
        entry.health = Some(out);
    }
    if (project.related.is_empty() || force)
        && let Ok(out) = run_related(runner, project, all_names, pipeline_dir)
    {
        entry.related = out;
    }

    entry
}

fn enrich_project(
    runner: &dyn PipelineRunner,
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
    let entry = build_entry(runner, project, cached, all_names, pipeline_dir, force);
    apply_entry(project, entry.clone());
    cache.set(slug, entry);
}

pub fn enrich(
    runner: &dyn PipelineRunner,
    projects: &mut [Project],
    pipeline_dir: &Path,
    cache_path: &Path,
    force: bool,
) -> Result<()> {
    let mut cache = EnrichCache::load(cache_path);
    let all_names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();

    for project in projects.iter_mut() {
        enrich_project(runner, project, &mut cache, &all_names, pipeline_dir, force);
    }

    cache.save(cache_path)?;
    Ok(())
}

#[cfg(test)]
struct MockPipelineRunner;

#[cfg(test)]
impl PipelineRunner for MockPipelineRunner {
    fn run(&self, pipeline: &Path, _input_json: &str) -> Result<serde_json::Value> {
        let name = pipeline
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        Ok(match name {
            "enrich_describe" => serde_json::json!({"description": "mock description"}),
            "enrich_classify" => serde_json::json!({"category": "mock category"}),
            "enrich_changelog" => serde_json::json!({"summary": "mock changelog"}),
            "enrich_health" => serde_json::json!({"status": "healthy"}),
            "enrich_related" => serde_json::json!({"related": ["other-project"]}),
            other => anyhow::bail!("unexpected pipeline: {other}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    fn empty_project(name: &str) -> Project {
        Project {
            name: name.to_string(),
            description: None,
            url: String::new(),
            kinds: vec![],
            language: None,
            pushed_at: None,
            version: None,
            stars: None,
            downloads: None,
            recent_commits: vec![crate::model::Commit {
                message: "initial commit".to_string(),
                date: chrono::Utc::now(),
            }],
            tags: vec![],
            topics: vec![],
            readme: None,
            category: None,
            changelog: None,
            health: None,
            related: vec![],
        }
    }

    #[test]
    fn enrich_fills_missing_fields_from_pipeline_runner() {
        let mut projects = vec![empty_project("demo")];
        let runner = MockPipelineRunner;
        let cache_path =
            std::env::temp_dir().join(format!("bazaar-enrich-test-{}.json", std::process::id()));
        let pipeline_dir = Path::new("examples");

        enrich(&runner, &mut projects, pipeline_dir, &cache_path, false).unwrap();

        assert_eq!(projects[0].description.as_deref(), Some("mock description"));
        assert_eq!(projects[0].category.as_deref(), Some("mock category"));
        assert_eq!(projects[0].changelog.as_deref(), Some("mock changelog"));
        assert_eq!(projects[0].related, vec!["other-project".to_string()]);
        assert!(projects[0].health.is_some());

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn enrich_skips_projects_that_already_have_data() {
        let mut project = empty_project("demo");
        project.description = Some("already set".to_string());
        project.category = Some("already set".to_string());
        project.changelog = Some("already set".to_string());
        project.related = vec!["x".to_string()];
        let mut projects = vec![project];
        let runner = MockPipelineRunner;
        let cache_path = std::env::temp_dir().join(format!(
            "bazaar-enrich-test-skip-{}.json",
            std::process::id()
        ));
        let pipeline_dir = Path::new("examples");

        enrich(&runner, &mut projects, pipeline_dir, &cache_path, false).unwrap();

        assert_eq!(projects[0].description.as_deref(), Some("already set"));
        let _ = std::fs::remove_file(&cache_path);
    }
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
    runner: &dyn PipelineRunner,
    pipeline_dir: &Path,
    file: &str,
    payload: serde_json::Value,
    result_key: &str,
) -> Result<String> {
    let out = runner.run(&pipeline_dir.join(file), &payload.to_string())?;
    Ok(out[result_key].as_str().unwrap_or("").to_string())
}

fn run_describe(
    runner: &dyn PipelineRunner,
    project: &Project,
    pipeline_dir: &Path,
) -> Result<String> {
    let commits = commit_messages(project);
    run_pipeline(
        runner,
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

fn run_classify(
    runner: &dyn PipelineRunner,
    project: &Project,
    pipeline_dir: &Path,
) -> Result<String> {
    let commits = commit_messages(project);
    run_pipeline(
        runner,
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

fn run_changelog(
    runner: &dyn PipelineRunner,
    project: &Project,
    pipeline_dir: &Path,
) -> Result<String> {
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
    let out = runner.run(
        &pipeline_dir.join("enrich_changelog.crux"),
        &input.to_string(),
    )?;
    Ok(out["summary"].as_str().unwrap_or("").to_string())
}

fn run_health(
    runner: &dyn PipelineRunner,
    project: &Project,
    pipeline_dir: &Path,
) -> Result<String> {
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
    let out = runner.run(&pipeline_dir.join("enrich_health.crux"), &input.to_string())?;
    Ok(out["status"].as_str().unwrap_or("").to_string())
}

fn run_related(
    runner: &dyn PipelineRunner,
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
    let out = runner.run(
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
