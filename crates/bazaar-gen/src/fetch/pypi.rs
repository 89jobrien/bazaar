use crate::model::{Kind, Project};
use crate::port::SourceFetcher;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

pub struct PypiFetcher {
    pub client: Client,
    pub packages: Vec<String>,
}

#[derive(Deserialize)]
struct PypiResp {
    info: PypiInfo,
}

#[derive(Deserialize)]
struct PypiInfo {
    name: String,
    version: String,
    summary: Option<String>,
    home_page: Option<String>,
    project_url: Option<String>,
}

#[async_trait::async_trait]
impl SourceFetcher for PypiFetcher {
    async fn fetch(&self) -> Result<Vec<Project>> {
        let mut projects = Vec::new();
        for pkg in &self.packages {
            let url = format!("https://pypi.org/pypi/{}/json", pkg);
            let resp = self.client.get(&url).send().await?;
            if resp.status() == 404 {
                eprintln!("warning: PyPI package '{pkg}' not found — skipping");
                continue;
            }
            if !resp.status().is_success() {
                eprintln!("warning: PyPI fetch for '{pkg}' failed ({}) — skipping", resp.status());
                continue;
            }
            let body: PypiResp = resp.json().await?;
            let url = body.info.project_url
                .or(body.info.home_page)
                .unwrap_or_else(|| format!("https://pypi.org/project/{}", body.info.name));
            projects.push(Project {
                name: body.info.name,
                description: body.info.summary,
                url,
                kinds: vec![Kind::PyPI],
                language: Some("Python".to_string()),
                pushed_at: None,
                version: Some(body.info.version),
                stars: None,
                downloads: None,
            });
        }
        Ok(projects)
    }
}
