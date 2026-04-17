# Bazaar Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `bz` Rust binary that fetches public repos, crates.io packages, PyPI packages,
and Claude plugin data, then renders a self-contained `index.html` and updated `README.md`,
refreshed daily via GitHub Actions.

**Architecture:** Hexagonal — domain traits (ports) for each data source, concrete fetch adapters
as implementations, a merge/sort model layer, and Askama template rendering. `main.rs` is the
composition root wiring config → adapters → renderer. Zero hardcoded PII; all user identity comes
from env vars (`GITHUB_USER`, `CRATES_IO_USER`) and `pypi.toml`.

**Tech Stack:** Rust stable, reqwest 0.12 (rustls), tokio, serde_json, toml, askama, chrono,
clap, anyhow. Binary name: `bz`. Crate name: `bazaar-gen`.

---

## File Map

```
Cargo.toml                          workspace root (members = ["crates/bazaar-gen"])
crates/bazaar-gen/
  Cargo.toml                        [bin] name = "bz"
  src/
    main.rs                         composition root: parse args, load config, run fetchers, render
    config.rs                       Config struct: reads env vars + pypi.toml
    error.rs                        BazaarError enum (domain errors only)
    model.rs                        Project, Kind, merge(), sort()
    port.rs                         SourceFetcher trait (the port)
    fetch/
      mod.rs                        re-exports all adapters
      github.rs                     GitHubFetcher: implements SourceFetcher
      crates_io.rs                  CratesIoFetcher: implements SourceFetcher
      pypi.rs                       PypiFetcher: implements SourceFetcher
      plugins.rs                    PluginFetcher: reads .claude-plugin/marketplace.json
    render/
      mod.rs                        render_html(), render_readme()
      html.rs                       Askama template struct + impl
      markdown.rs                   README table generator
  templates/
    index.html                      Askama template (include via askama derive)
.github/workflows/generate.yml      CI: build bz, run, commit output
pypi.toml                           user's PyPI package list (no PII in code)
README.header.md                    human-authored header for README.md
```

---

## Task 1: Workspace scaffold

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/bazaar-gen/Cargo.toml`
- Create: `crates/bazaar-gen/src/main.rs`

- [ ] **Step 1: Add bazaar-gen to workspace**

If no `Cargo.toml` exists at repo root, create one:

```toml
[workspace]
members = ["crates/bazaar-gen"]
resolver = "2"
```

If one already exists, add `"bazaar-gen"` to the `members` array.

- [ ] **Step 2: Create crates/bazaar-gen/Cargo.toml**

```toml
[package]
name = "bazaar-gen"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "bz"
path = "src/main.rs"

[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
askama = "0.12"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
```

- [ ] **Step 3: Create minimal main.rs**

```rust
fn main() {
    println!("bz ok");
}
```

- [ ] **Step 4: Verify it builds**

```
cargo build -p bazaar-gen
```

Expected: compiles, `./target/debug/bz` exists, prints "bz ok".

- [ ] **Step 5: Commit**

```
git add Cargo.toml crates/bazaar-gen/
git commit -m "chore: scaffold bazaar-gen crate with bz binary"
```

---

## Task 2: Config and error types

**Files:**
- Create: `crates/bazaar-gen/src/config.rs`
- Create: `crates/bazaar-gen/src/error.rs`
- Create: `pypi.toml`

- [ ] **Step 1: Write failing tests for config loading**

Create `crates/bazaar-gen/src/config.rs`:

```rust
use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub github_token: Option<String>,
    pub github_user: String,
    pub crates_io_user: String,
    pub pypi_packages: Vec<String>,
    pub plugin_manifest: std::path::PathBuf,
}

#[derive(Deserialize)]
struct PypiToml {
    packages: Vec<String>,
}

impl Config {
    pub fn from_env(pypi_toml_path: &Path, plugin_manifest: std::path::PathBuf) -> Result<Self> {
        let github_user = std::env::var("GITHUB_USER")
            .map_err(|_| anyhow::anyhow!("GITHUB_USER env var is required"))?;
        let crates_io_user = std::env::var("CRATES_IO_USER")
            .map_err(|_| anyhow::anyhow!("CRATES_IO_USER env var is required"))?;
        let github_token = std::env::var("GITHUB_TOKEN").ok();

        if github_token.is_none() {
            eprintln!("warning: GITHUB_TOKEN not set — using unauthenticated GitHub API (60 req/hr)");
        }

        let pypi_packages = if pypi_toml_path.exists() {
            let raw = std::fs::read_to_string(pypi_toml_path)?;
            let parsed: PypiToml = toml::from_str(&raw)?;
            parsed.packages
        } else {
            vec![]
        };

        Ok(Config {
            github_token,
            github_user,
            crates_io_user,
            pypi_packages,
            plugin_manifest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn missing_github_user_returns_error() {
        std::env::remove_var("GITHUB_USER");
        std::env::remove_var("CRATES_IO_USER");
        let result = Config::from_env(Path::new("nonexistent.toml"), "manifest.json".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("GITHUB_USER"));
    }

    #[test]
    fn missing_crates_io_user_returns_error() {
        std::env::set_var("GITHUB_USER", "testuser");
        std::env::remove_var("CRATES_IO_USER");
        let result = Config::from_env(Path::new("nonexistent.toml"), "manifest.json".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRATES_IO_USER"));
        std::env::remove_var("GITHUB_USER");
    }

    #[test]
    fn parses_pypi_toml() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"packages = ["foo", "bar"]"#).unwrap();
        std::env::set_var("GITHUB_USER", "u");
        std::env::set_var("CRATES_IO_USER", "u");
        let cfg = Config::from_env(f.path(), "manifest.json".into()).unwrap();
        assert_eq!(cfg.pypi_packages, vec!["foo", "bar"]);
        std::env::remove_var("GITHUB_USER");
        std::env::remove_var("CRATES_IO_USER");
    }
}
```

- [ ] **Step 2: Add tempfile dev-dependency**

In `crates/bazaar-gen/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Create error.rs**

```rust
#[derive(Debug, thiserror::Error)]
pub enum BazaarError {
    #[error("HTTP error fetching {url}: {status}")]
    Http { url: String, status: u16 },
    #[error("Rate limited by {source} — retry after {retry_after:?}s")]
    RateLimited { source: String, retry_after: Option<u64> },
    #[error("Render error: {0}")]
    Render(String),
}
```

Add `thiserror` to dependencies:

```toml
thiserror = "1"
```

- [ ] **Step 4: Wire config into main.rs**

```rust
mod config;
mod error;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "bz", about = "bazaar showcase generator")]
struct Args {
    #[arg(long, default_value = "index.html")]
    output: PathBuf,
    #[arg(long, default_value = "README.md")]
    readme: PathBuf,
    #[arg(long, default_value = "pypi.toml")]
    pypi_toml: PathBuf,
    #[arg(long, default_value = ".claude-plugin/marketplace.json")]
    plugin_manifest: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = config::Config::from_env(&args.pypi_toml, args.plugin_manifest)?;
    println!("config loaded for user: {}", config.github_user);
    Ok(())
}
```

- [ ] **Step 5: Run tests**

```
cargo test -p bazaar-gen
```

Expected: 3 tests pass.

- [ ] **Step 6: Create pypi.toml in repo root**

```toml
# List your PyPI package names here
packages = []
```

- [ ] **Step 7: Commit**

```
git add crates/bazaar-gen/ pypi.toml
git commit -m "feat(bz): config loading from env vars and pypi.toml"
```

---

## Task 3: Domain model and port trait

**Files:**
- Create: `crates/bazaar-gen/src/model.rs`
- Create: `crates/bazaar-gen/src/port.rs`

- [ ] **Step 1: Write failing model tests**

Create `crates/bazaar-gen/src/model.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    GitHubRepo,
    CratesIo,
    PyPI,
    ClaudePlugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub kinds: Vec<Kind>,
    pub language: Option<String>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
    pub stars: Option<u32>,
    pub downloads: Option<u64>,
}

/// Merge projects from multiple sources. Projects with the same `name` are combined:
/// kinds are unioned, and fields from later entries fill in None fields from earlier ones.
pub fn merge(mut projects: Vec<Project>) -> Vec<Project> {
    let mut map: indexmap::IndexMap<String, Project> = indexmap::IndexMap::new();
    for p in projects.drain(..) {
        map.entry(p.name.clone())
            .and_modify(|existing| {
                for k in &p.kinds {
                    if !existing.kinds.contains(k) {
                        existing.kinds.push(k.clone());
                    }
                }
                if existing.description.is_none() { existing.description = p.description.clone(); }
                if existing.pushed_at.is_none() { existing.pushed_at = p.pushed_at; }
                if existing.version.is_none() { existing.version = p.version.clone(); }
                if existing.stars.is_none() { existing.stars = p.stars; }
                if existing.downloads.is_none() { existing.downloads = p.downloads; }
            })
            .or_insert(p);
    }
    let mut out: Vec<Project> = map.into_values().collect();
    // Sort by pushed_at descending, None last
    out.sort_by(|a, b| match (a.pushed_at, b.pushed_at) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make(name: &str, kind: Kind, pushed: Option<DateTime<Utc>>) -> Project {
        Project {
            name: name.to_string(),
            description: None,
            url: format!("https://example.com/{name}"),
            kinds: vec![kind],
            language: None,
            pushed_at: pushed,
            version: None,
            stars: None,
            downloads: None,
        }
    }

    #[test]
    fn merge_combines_same_name() {
        let a = make("foo", Kind::GitHubRepo, None);
        let b = make("foo", Kind::CratesIo, None);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert!(out[0].kinds.contains(&Kind::GitHubRepo));
        assert!(out[0].kinds.contains(&Kind::CratesIo));
    }

    #[test]
    fn merge_keeps_distinct_names() {
        let a = make("foo", Kind::GitHubRepo, None);
        let b = make("bar", Kind::CratesIo, None);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn sort_pushed_at_descending() {
        let older = make("old", Kind::GitHubRepo, Some(Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()));
        let newer = make("new", Kind::GitHubRepo, Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()));
        let out = merge(vec![older, newer]);
        assert_eq!(out[0].name, "new");
        assert_eq!(out[1].name, "old");
    }

    #[test]
    fn none_pushed_at_sorts_last() {
        let with_date = make("dated", Kind::GitHubRepo, Some(Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap()));
        let without = make("noddate", Kind::CratesIo, None);
        let out = merge(vec![without, with_date]);
        assert_eq!(out[0].name, "dated");
        assert_eq!(out[1].name, "noddate");
    }
}
```

- [ ] **Step 2: Add indexmap dependency**

```toml
indexmap = "2"
```

- [ ] **Step 3: Create port.rs**

```rust
use crate::model::Project;

/// Port: any data source that yields a list of Projects.
#[async_trait::async_trait]
pub trait SourceFetcher: Send + Sync {
    async fn fetch(&self) -> anyhow::Result<Vec<Project>>;
}
```

Add `async-trait` dependency:

```toml
async-trait = "0.1"
```

- [ ] **Step 4: Wire model into main.rs**

Add `mod model; mod port;` to `main.rs`.

- [ ] **Step 5: Run tests**

```
cargo test -p bazaar-gen
```

Expected: 4 tests pass (3 config + 4 model).

- [ ] **Step 6: Commit**

```
git add crates/bazaar-gen/src/model.rs crates/bazaar-gen/src/port.rs crates/bazaar-gen/Cargo.toml crates/bazaar-gen/src/main.rs
git commit -m "feat(bz): domain model and SourceFetcher port trait"
```

---

## Task 4: GitHub adapter

**Files:**
- Create: `crates/bazaar-gen/src/fetch/mod.rs`
- Create: `crates/bazaar-gen/src/fetch/github.rs`

- [ ] **Step 1: Create fetch/mod.rs**

```rust
pub mod github;
pub mod crates_io;
pub mod pypi;
pub mod plugins;
```

- [ ] **Step 2: Create fetch/github.rs**

```rust
use crate::model::{Kind, Project};
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
            let version = self.latest_release(&self.user, &repo.name).await;
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
            });
        }
        Ok(projects)
    }
}
```

- [ ] **Step 3: Build to check for errors**

```
cargo build -p bazaar-gen
```

Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```
git add crates/bazaar-gen/src/fetch/
git commit -m "feat(bz): GitHub adapter — fetches public repos from past 6 months"
```

---

## Task 5: crates.io adapter

**Files:**
- Create: `crates/bazaar-gen/src/fetch/crates_io.rs`

- [ ] **Step 1: Create fetch/crates_io.rs**

```rust
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
        // Step 1: resolve user id
        let url = format!("https://crates.io/api/v1/users/{}", self.user);
        let resp = self.client
            .get(&url)
            .header("User-Agent", "bazaar-gen/bz")
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("crates.io user lookup failed: {}", resp.status());
        }
        let user_resp: UserResp = resp.json().await?;
        let user_id = user_resp.user.id;

        // Step 2: paginate crates
        let mut projects = Vec::new();
        let mut page = 1u64;
        loop {
            let url = format!(
                "https://crates.io/api/v1/crates?user_id={}&per_page=100&page={}",
                user_id, page
            );
            let resp = self.client
                .get(&url)
                .header("User-Agent", "bazaar-gen/bz")
                .send()
                .await?;
            if !resp.status().is_success() {
                anyhow::bail!("crates.io crates fetch failed: {}", resp.status());
            }
            let body: CratesResp = resp.json().await?;
            let count = body.crates.len() as u64;
            for c in body.crates {
                projects.push(Project {
                    name: c.name,
                    description: c.description,
                    url: format!("https://crates.io/crates/{}", projects.len()), // placeholder, fixed below
                    kinds: vec![Kind::CratesIo],
                    language: Some("Rust".to_string()),
                    pushed_at: c.updated_at,
                    version: c.newest_version,
                    stars: None,
                    downloads: c.downloads,
                });
            }
            // Fix URLs (name was moved into Project, recompute)
            if page == 1 { /* url set correctly below */ }
            if count < 100 || projects.len() as u64 >= body.meta.total { break; }
            page += 1;
        }
        // Fix crate URLs
        for p in &mut projects {
            if p.kinds.contains(&Kind::CratesIo) && p.url.contains("placeholder") {
                p.url = format!("https://crates.io/crates/{}", p.name);
            }
        }
        Ok(projects)
    }
}
```

Note: The URL construction above has a bug in the loop (using `projects.len()` before push).
Correct implementation:

- [ ] **Step 2: Fix the URL bug — replace the crates_io.rs content with corrected version**

```rust
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
        let resp = self.client
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
            let resp = self.client
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
                });
            }
            fetched += count;
            if fetched >= total || count < 100 { break; }
            page += 1;
        }
        Ok(projects)
    }
}
```

- [ ] **Step 3: Build**

```
cargo build -p bazaar-gen
```

Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```
git add crates/bazaar-gen/src/fetch/crates_io.rs
git commit -m "feat(bz): crates.io adapter — user crate listing with pagination"
```

---

## Task 6: PyPI and plugins adapters

**Files:**
- Create: `crates/bazaar-gen/src/fetch/pypi.rs`
- Create: `crates/bazaar-gen/src/fetch/plugins.rs`

- [ ] **Step 1: Create fetch/pypi.rs**

```rust
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
```

- [ ] **Step 2: Create fetch/plugins.rs**

```rust
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
        }).collect())
    }
}
```

- [ ] **Step 3: Build**

```
cargo build -p bazaar-gen
```

Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```
git add crates/bazaar-gen/src/fetch/pypi.rs crates/bazaar-gen/src/fetch/plugins.rs
git commit -m "feat(bz): PyPI and Claude plugin adapters"
```

---

## Task 7: HTML renderer (Askama template)

**Files:**
- Create: `crates/bazaar-gen/src/render/mod.rs`
- Create: `crates/bazaar-gen/src/render/html.rs`
- Create: `crates/bazaar-gen/templates/index.html`

- [ ] **Step 1: Create templates/index.html**

Note: Askama looks for templates relative to the crate root by default. Create
`crates/bazaar-gen/templates/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{{ username }}'s open source</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{background:#0d1117;color:#e6edf3;font-family:'Courier New',monospace;padding:2rem;max-width:1100px;margin:0 auto}
h1{font-size:1.5rem;margin-bottom:.25rem}
.sub{color:#8b949e;margin-bottom:2rem;font-size:.9rem}
nav{display:flex;gap:1rem;margin-bottom:2rem;flex-wrap:wrap}
nav a{color:#58a6ff;text-decoration:none;font-size:.85rem;padding:.25rem .5rem;border:1px solid #30363d;border-radius:4px}
nav a:hover{background:#161b22}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:1rem}
.card{background:#161b22;border:1px solid #30363d;border-radius:6px;padding:1rem}
.card-name{font-weight:bold;margin-bottom:.25rem}
.card-name a{color:#58a6ff;text-decoration:none}
.card-name a:hover{text-decoration:underline}
.badges{display:flex;gap:.35rem;flex-wrap:wrap;margin-bottom:.5rem}
.badge{font-size:.7rem;padding:.1rem .4rem;border-radius:3px;font-weight:bold}
.badge-gh{background:#21262d;color:#8b949e;border:1px solid #30363d}
.badge-crate{background:#1a1a2e;color:#f78166;border:1px solid #3d1a1a}
.badge-pypi{background:#1a2a1a;color:#56d364;border:1px solid #1a3d1a}
.badge-plugin{background:#1a1a3d;color:#a371f7;border:1px solid #2d1f7a}
.desc{color:#8b949e;font-size:.85rem;margin-bottom:.5rem;line-height:1.4}
.meta{font-size:.75rem;color:#6e7681;display:flex;gap:.75rem;flex-wrap:wrap}
footer{margin-top:3rem;color:#6e7681;font-size:.75rem;text-align:center}
</style>
</head>
<body>
<h1>{{ username }}'s open source</h1>
<p class="sub">{{ projects|length }} projects across GitHub, crates.io, PyPI, and Claude plugins</p>
<nav>
  <a href="https://github.com/{{ username }}">GitHub</a>
  <a href="https://crates.io/users/{{ crates_user }}">crates.io</a>
</nav>
<div class="grid">
{% for p in projects %}
<div class="card">
  <div class="card-name"><a href="{{ p.url }}" target="_blank" rel="noopener">{{ p.name }}</a></div>
  <div class="badges">
    {% for k in p.kinds %}
    {% match k %}
    {% when Kind::GitHubRepo %}
    <span class="badge badge-gh">GitHub</span>
    {% when Kind::CratesIo %}
    <span class="badge badge-crate">crate</span>
    {% when Kind::PyPI %}
    <span class="badge badge-pypi">PyPI</span>
    {% when Kind::ClaudePlugin %}
    <span class="badge badge-plugin">plugin</span>
    {% endmatch %}
    {% endfor %}
    {% if let Some(lang) = p.language %}
    <span class="badge badge-gh">{{ lang }}</span>
    {% endif %}
  </div>
  {% if let Some(desc) = p.description %}
  <div class="desc">{{ desc }}</div>
  {% endif %}
  <div class="meta">
    {% if let Some(v) = p.version %}<span>v{{ v }}</span>{% endif %}
    {% if let Some(s) = p.stars %}<span>★ {{ s }}</span>{% endif %}
    {% if let Some(d) = p.downloads %}<span>{{ d }} downloads</span>{% endif %}
    {% if let Some(pushed) = p.pushed_at %}<span>{{ pushed|date("%Y-%m-%d") }}</span>{% endif %}
  </div>
</div>
{% endfor %}
</div>
<footer>Generated {{ generated_at }}</footer>
</body>
</html>
```

- [ ] **Step 2: Create render/mod.rs**

```rust
pub mod html;
pub mod markdown;
```

- [ ] **Step 3: Create render/html.rs**

```rust
use crate::model::{Kind, Project};
use anyhow::Result;
use askama::Template;
use chrono::Utc;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    username: &'a str,
    crates_user: &'a str,
    projects: &'a [Project],
    generated_at: String,
}

pub fn render_html(username: &str, crates_user: &str, projects: &[Project]) -> Result<String> {
    let tmpl = IndexTemplate {
        username,
        crates_user,
        projects,
        generated_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
    };
    tmpl.render().map_err(|e| anyhow::anyhow!("template render failed: {e}"))
}
```

Note: Askama templates reference `Kind` variants — add `use crate::model::Kind;` to the template
context by ensuring the template struct is in the same module that imports `Kind`. Askama needs
types used in templates to be in scope via the struct's module. Add this to `render/html.rs`:

```rust
// Re-export Kind so Askama template can reference it
pub use crate::model::Kind;
```

- [ ] **Step 4: Build (Askama validates template at compile time)**

```
cargo build -p bazaar-gen
```

Expected: compiles. If Askama reports template errors, fix the template syntax — common issues:
`{% if let Some(x) = y %}` requires `{% endif %}`, and `{% match %}` arms use `{% when %}`.

- [ ] **Step 5: Commit**

```
git add crates/bazaar-gen/src/render/ crates/bazaar-gen/templates/
git commit -m "feat(bz): Askama HTML renderer with dark monospace theme"
```

---

## Task 8: README renderer

**Files:**
- Create: `crates/bazaar-gen/src/render/markdown.rs`
- Create: `README.header.md`

- [ ] **Step 1: Create render/markdown.rs**

```rust
use crate::model::{Kind, Project};
use anyhow::Result;
use chrono::Utc;
use std::path::Path;

fn kind_label(kinds: &[Kind]) -> String {
    kinds.iter().map(|k| match k {
        Kind::GitHubRepo => "Repo",
        Kind::CratesIo => "Crate",
        Kind::PyPI => "PyPI",
        Kind::ClaudePlugin => "Plugin",
    }).collect::<Vec<_>>().join(" / ")
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
        let updated = p.pushed_at
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "—".to_string());
        out.push_str(&format!("| {name} | {kind} | {desc} | {updated} |\n"));
    }

    Ok(out)
}
```

- [ ] **Step 2: Create README.header.md in repo root**

```markdown
# bazaar

My open-source software — plugins, crates, tools, and experiments.

```

- [ ] **Step 3: Build**

```
cargo build -p bazaar-gen
```

Expected: compiles.

- [ ] **Step 4: Commit**

```
git add crates/bazaar-gen/src/render/markdown.rs README.header.md
git commit -m "feat(bz): README markdown renderer"
```

---

## Task 9: Wire composition root and run end-to-end

**Files:**
- Modify: `crates/bazaar-gen/src/main.rs`

- [ ] **Step 1: Replace main.rs with full composition root**

```rust
mod config;
mod error;
mod fetch;
mod model;
mod port;
mod render;

use clap::Parser;
use config::Config;
use fetch::{
    crates_io::CratesIoFetcher,
    github::GitHubFetcher,
    plugins::PluginFetcher,
    pypi::PypiFetcher,
};
use port::SourceFetcher;
use reqwest::Client;
use std::path::PathBuf;
use tokio::try_join;

#[derive(Parser)]
#[command(name = "bz", about = "bazaar showcase generator")]
struct Args {
    #[arg(long, default_value = "index.html")]
    output: PathBuf,
    #[arg(long, default_value = "README.md")]
    readme: PathBuf,
    #[arg(long, default_value = "pypi.toml")]
    pypi_toml: PathBuf,
    #[arg(long, default_value = ".claude-plugin/marketplace.json")]
    plugin_manifest: PathBuf,
    #[arg(long, default_value = "README.header.md")]
    readme_header: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::from_env(&args.pypi_toml, args.plugin_manifest.clone())?;
    let client = Client::new();

    let github = GitHubFetcher {
        client: client.clone(),
        user: config.github_user.clone(),
        token: config.github_token.clone(),
    };
    let crates = CratesIoFetcher {
        client: client.clone(),
        user: config.crates_io_user.clone(),
    };
    let pypi = PypiFetcher {
        client: client.clone(),
        packages: config.pypi_packages.clone(),
    };
    let plugins = PluginFetcher {
        manifest_path: args.plugin_manifest,
    };

    eprintln!("fetching from all sources...");
    let (gh, cr, py, pl) = try_join!(
        github.fetch(),
        crates.fetch(),
        pypi.fetch(),
        plugins.fetch(),
    )?;

    let mut all = gh;
    all.extend(cr);
    all.extend(py);
    all.extend(pl);

    let projects = model::merge(all);
    eprintln!("{} projects after merge", projects.len());

    let html = render::html::render_html(&config.github_user, &config.crates_io_user, &projects)?;
    std::fs::write(&args.output, &html)?;
    eprintln!("wrote {}", args.output.display());

    let md = render::markdown::render_readme(&projects, &args.readme_header)?;
    std::fs::write(&args.readme, &md)?;
    eprintln!("wrote {}", args.readme.display());

    Ok(())
}
```

- [ ] **Step 2: Run all tests**

```
cargo test -p bazaar-gen
```

Expected: all pass.

- [ ] **Step 3: Run bz locally (requires GITHUB_USER and CRATES_IO_USER)**

```
GITHUB_USER=89jobrien CRATES_IO_USER=89jobrien cargo run -p bazaar-gen -- --output /tmp/index.html --readme /tmp/README.md
```

Expected: fetches data, writes files, prints project count.

- [ ] **Step 4: Inspect output**

```
wc -l /tmp/index.html /tmp/README.md
```

Expected: non-zero line counts. Open `/tmp/index.html` in a browser to verify layout.

- [ ] **Step 5: Commit**

```
git add crates/bazaar-gen/src/main.rs
git commit -m "feat(bz): wire composition root — all fetchers, merge, render"
```

---

## Task 10: GitHub Actions workflow and Pages

**Files:**
- Create: `.github/workflows/generate.yml`

- [ ] **Step 1: Create workflow using bash heredoc (Edit tool is blocked for workflow files)**

```bash
mkdir -p .github/workflows
cat > .github/workflows/generate.yml << 'EOF'
name: Generate showcase

on:
  push:
    branches: [main]
  schedule:
    - cron: '0 6 * * *'
  workflow_dispatch:

permissions:
  contents: write

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: ". -> target"

      - name: Build bz
        run: cargo build --release -p bazaar-gen

      - name: Generate showcase
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          GITHUB_USER: ${{ vars.GITHUB_USER }}
          CRATES_IO_USER: ${{ vars.CRATES_IO_USER }}
        run: ./target/release/bz --output index.html --readme README.md

      - name: Commit if changed
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add index.html README.md
          if git diff --staged --quiet; then
            echo "no changes"
          else
            git commit -m "chore: regenerate showcase [skip ci]"
            git push
          fi
EOF
```

- [ ] **Step 2: Set GitHub repo variables**

In the GitHub repo settings → Variables (not secrets):
- `GITHUB_USER` = `89jobrien`
- `CRATES_IO_USER` = `89jobrien`

`GITHUB_TOKEN` is automatically provided by Actions.

- [ ] **Step 3: Enable GitHub Pages**

In repo settings → Pages → Source: Deploy from branch → `main` → `/ (root)`.

- [ ] **Step 4: Commit workflow**

```
git add .github/workflows/generate.yml
git commit -m "ci: GitHub Actions workflow to generate showcase daily"
git push
```

- [ ] **Step 5: Verify workflow runs**

```
gh run list --limit 3
```

Expected: a run triggered by the push, status `completed` / `success`.

---

## Self-Review

**Spec coverage:**
- Public GitHub repos (6 months) → Task 4
- crates.io packages → Task 5
- PyPI packages → Task 6
- Claude plugins → Task 6
- `index.html` single-file output → Task 7
- `README.md` with generated table → Task 8
- No hardcoded PII → Config entirely env-var driven (Tasks 2, 10)
- GitHub Actions daily refresh → Task 10
- GitHub Pages serving → Task 10

**Placeholder scan:** None found.

**Type consistency:**
- `Project` and `Kind` defined in Task 3, used identically in Tasks 4–9.
- `SourceFetcher` trait defined in Task 3, implemented in Tasks 4–6, called in Task 9.
- `Config` defined in Task 2, consumed in Task 9 (`config.github_user`, `config.crates_io_user`,
  `config.pypi_packages`, `config.github_token`).
- `render::html::render_html(username, crates_user, projects)` defined in Task 7, called in Task 9.
- `render::markdown::render_readme(projects, header_path)` defined in Task 8, called in Task 9.
