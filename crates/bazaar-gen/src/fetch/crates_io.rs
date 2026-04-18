use crate::model::{Kind, Project};
use crate::port::SourceFetcher;
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

pub struct CratesIoFetcher {
    pub client: Client,
    pub user: String,
}

#[derive(Deserialize)]
struct UserResp {
    user: UserInner,
}

#[derive(Deserialize)]
struct UserInner {
    id: u64,
}

#[derive(Deserialize)]
struct CratesResp {
    crates: Vec<CrateEntry>,
    meta: Meta,
}

#[derive(Deserialize)]
struct Meta {
    total: u64,
}

#[derive(Deserialize)]
struct CrateEntry {
    name: String,
    description: Option<String>,
    updated_at: Option<DateTime<Utc>>,
    newest_version: Option<String>,
    downloads: Option<u64>,
}

#[async_trait::async_trait]
impl SourceFetcher for CratesIoFetcher {
    async fn fetch(&self) -> Result<Vec<Project>> {
        let url = format!("https://crates.io/api/v1/users/{}", self.user);
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "bazaar-gen/bz")
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("crates.io user lookup failed: {}", resp.status());
        }
        let user_resp: UserResp = resp.json().await?;
        let user_id = user_resp.user.id;

        let mut projects = Vec::new();
        let mut page = 1u64;
        let mut fetched = 0u64;
        loop {
            let url = format!(
                "https://crates.io/api/v1/crates?user_id={}&per_page=100&page={}",
                user_id, page
            );
            let resp = self
                .client
                .get(&url)
                .header("User-Agent", "bazaar-gen/bz")
                .send()
                .await?;
            if !resp.status().is_success() {
                anyhow::bail!("crates.io crates fetch failed: {}", resp.status());
            }
            let body: CratesResp = resp.json().await?;
            let total = body.meta.total;
            let count = body.crates.len() as u64;
            for c in body.crates {
                projects.push(Project {
                    url: format!("https://crates.io/crates/{}", c.name),
                    name: c.name,
                    description: c.description,
                    kinds: vec![Kind::CratesIo],
                    language: Some("Rust".to_string()),
                    pushed_at: c.updated_at,
                    version: c.newest_version,
                    stars: None,
                    downloads: c.downloads,
                    recent_commits: vec![],
                });
            }
            fetched += count;
            if fetched >= total || count < 100 {
                break;
            }
            page += 1;
        }
        Ok(projects)
    }
}
