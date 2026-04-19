use crate::model::{Kind, Project};
use crate::port::SourceFetcher;
use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

pub struct PluginFetcher {
    pub manifest_path: PathBuf,
}

#[derive(Deserialize)]
struct Manifest {
    plugins: Vec<PluginEntry>,
}

#[derive(Deserialize)]
struct PluginEntry {
    name: String,
    description: String,
    source: PluginSource,
}

#[derive(Deserialize)]
struct PluginSource {
    repo: String,
}

#[async_trait::async_trait]
impl SourceFetcher for PluginFetcher {
    async fn fetch(&self) -> Result<Vec<Project>> {
        if !self.manifest_path.exists() {
            return Ok(vec![]);
        }
        let raw = tokio::fs::read_to_string(&self.manifest_path).await?;
        let manifest: Manifest = serde_json::from_str(&raw)?;
        Ok(manifest.plugins.into_iter().map(|p| Project {
            url: format!("https://github.com/{}", p.source.repo),
            name: p.name,
            description: Some(p.description),
            kinds: vec![Kind::ClaudePlugin],
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
        }).collect())
    }
}
