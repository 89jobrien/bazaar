use crate::model::{Commit, Kind, Project};
use crate::port::SourceFetcher;
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

pub struct GitHubFetcher {
    pub client: Client,
    pub user: String,
    pub token: Option<String>,
}

#[derive(Deserialize)]
struct Repo {
    name: String,
    description: Option<String>,
    html_url: String,
    language: Option<String>,
    pushed_at: Option<DateTime<Utc>>,
    stargazers_count: u32,
    archived: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

#[derive(Deserialize)]
struct CommitItem {
    commit: CommitDetail,
}

#[derive(Deserialize)]
struct CommitDetail {
    message: String,
    author: CommitAuthor,
}

#[derive(Deserialize)]
struct CommitAuthor {
    date: Option<DateTime<Utc>>,
}

impl GitHubFetcher {
    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        let req = self.client
            .get(url)
            .header("User-Agent", "bazaar-gen/bz")
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = &self.token {
            req.bearer_auth(token)
        } else {
            req
        }
    }

    async fn recent_commits(&self, owner: &str, repo: &str) -> Vec<Commit> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/commits?per_page=3"
        );
        let resp = match self.request(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return vec![],
        };
        let items: Vec<CommitItem> = match resp.json().await {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        items.into_iter().filter_map(|item| {
            let date = item.commit.author.date?;
            let message = item.commit.message.lines().next()?.to_string();
            Some(Commit { message, date })
        }).collect()
    }

    async fn latest_release(&self, owner: &str, repo: &str) -> Option<String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
        let resp = self.request(&url).send().await.ok()?;
        if resp.status() == 404 { return None; }
        let release: Release = resp.json().await.ok()?;
        Some(release.tag_name)
    }
}

#[async_trait::async_trait]
impl SourceFetcher for GitHubFetcher {
    async fn fetch(&self) -> Result<Vec<Project>> {
        let cutoff = Utc::now() - chrono::Duration::days(180);
        let url = format!(
            "https://api.github.com/users/{}/repos?type=public&sort=pushed&per_page=100",
            self.user
        );
        let resp = self.request(&url).send().await?;
        let status = resp.status();
        if status == 403 || status == 429 {
            anyhow::bail!("GitHub rate limit hit ({})", status);
        }
        if !status.is_success() {
            anyhow::bail!("GitHub API error: {}", status);
        }
        let repos: Vec<Repo> = resp.json().await?;
        let mut projects = Vec::new();
        for repo in repos {
            if repo.archived { continue; }
            let pushed = repo.pushed_at.unwrap_or(Utc::now());
            if pushed < cutoff { continue; }
            let (version, recent_commits) = tokio::join!(
                self.latest_release(&self.user, &repo.name),
                self.recent_commits(&self.user, &repo.name),
            );
            projects.push(Project {
                name: repo.name,
                description: repo.description,
                url: repo.html_url,
                kinds: vec![Kind::GitHubRepo],
                language: repo.language,
                pushed_at: repo.pushed_at,
                version,
                stars: if repo.stargazers_count > 0 { Some(repo.stargazers_count) } else { None },
                downloads: None,
                recent_commits,
                tags: vec![],
            });
        }
        Ok(projects)
    }
}
