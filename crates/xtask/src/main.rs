use anyhow::{Context, Result, bail};
use chrono::Months;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "xtask", about = "bazaar maintenance tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fetch public Rust repos from GitHub and write repos.json
    FetchRepos,
    /// Update examples/ccusage.json with fresh ccusage data
    UpdateUsage,
    /// Archive a local ~/dev project to the Extreme SSD
    Archive {
        /// Name of the project directory under ~/dev/
        project: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::FetchRepos => fetch_repos(),
        Cmd::UpdateUsage => update_usage(),
        Cmd::Archive { project } => archive(&project),
    }
}

/// Workspace root — two levels above crates/xtask.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist")
}

/// Write `content` to `path` atomically (tempfile + rename, same directory).
fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let dir = path.parent().context("output path has no parent")?;
    std::fs::create_dir_all(dir)?;
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    std::fs::write(tmp.path(), content)?;
    tmp.persist(path)?;
    Ok(())
}

// ── fetch-repos ───────────────────────────────────────────────────────────────

const OWNER: &str = "89jobrien";

/// Raw shape returned by `gh repo list --json`.
#[derive(Deserialize)]
struct GhRepo {
    name: String,
    description: Option<String>,
    url: String,
    #[serde(rename = "pushedAt")]
    pushed_at: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "primaryLanguage")]
    primary_language: Option<GhLanguage>,
    #[serde(rename = "repositoryTopics")]
    repository_topics: Vec<GhTopic>,
    #[serde(rename = "stargazerCount")]
    stargazer_count: u32,
    #[serde(rename = "licenseInfo")]
    license_info: Option<GhLicense>,
    #[serde(rename = "latestRelease")]
    latest_release: Option<GhRelease>,
    #[serde(rename = "homepageUrl")]
    homepage_url: Option<String>,
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<GhBranchRef>,
}

#[derive(Deserialize)]
struct GhLanguage {
    name: String,
}
#[derive(Deserialize)]
struct GhTopic {
    topic: GhTopicInner,
}
#[derive(Deserialize)]
struct GhTopicInner {
    name: String,
}
#[derive(Deserialize)]
struct GhLicense {
    #[serde(rename = "spdxId")]
    spdx_id: String,
}
#[derive(Deserialize)]
struct GhRelease {
    #[serde(rename = "tagName")]
    tag_name: String,
}
#[derive(Deserialize)]
struct GhBranchRef {
    name: String,
}

/// Output shape written to repos.json.
#[derive(Serialize)]
struct Repo {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    url: String,
    pushed_at: String,
    created_at: String,
    topics: Vec<String>,
    stars: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_branch: Option<String>,
}

fn fetch_repos() -> Result<()> {
    let fields = [
        "name",
        "description",
        "url",
        "pushedAt",
        "createdAt",
        "primaryLanguage",
        "repositoryTopics",
        "stargazerCount",
        "licenseInfo",
        "latestRelease",
        "homepageUrl",
        "defaultBranchRef",
    ]
    .join(",");

    eprintln!("fetching repos for {OWNER}...");
    let out = Command::new("gh")
        .args([
            "repo",
            "list",
            OWNER,
            "--visibility=public",
            "--limit",
            "100",
            "--json",
            &fields,
        ])
        .output()
        .context("gh not found — install GitHub CLI")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("gh repo list failed: {stderr}");
    }

    let raw: Vec<GhRepo> = serde_json::from_slice(&out.stdout).context("parsing gh output")?;

    // Filter: pushed in the last 3 months, Rust repos only.
    let cutoff = chrono::Utc::now()
        .checked_sub_months(Months::new(3))
        .expect("valid date")
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let mut repos: Vec<Repo> = raw
        .into_iter()
        .filter(|r| {
            r.pushed_at > cutoff
                && r.primary_language
                    .as_ref()
                    .is_some_and(|l| l.name == "Rust")
        })
        .map(|r| Repo {
            name: r.name,
            description: r.description.filter(|d| !d.is_empty()),
            url: r.url,
            pushed_at: r.pushed_at,
            created_at: r.created_at,
            topics: r
                .repository_topics
                .into_iter()
                .map(|t| t.topic.name)
                .collect(),
            stars: r.stargazer_count,
            license: r.license_info.map(|l| l.spdx_id),
            latest_release: r.latest_release.map(|r| r.tag_name),
            homepage: r.homepage_url.filter(|h| !h.is_empty()),
            default_branch: r.default_branch_ref.map(|b| b.name),
        })
        .collect();

    // Sort by pushed_at descending (lexicographic ISO 8601 comparison is correct).
    repos.sort_by(|a, b| b.pushed_at.cmp(&a.pushed_at));

    let out_path = workspace_root().join("repos.json");
    let json = serde_json::to_vec_pretty(&repos)?;
    atomic_write(&out_path, &json)?;
    eprintln!("wrote {} repos to {}", repos.len(), out_path.display());
    Ok(())
}

// ── update-usage ──────────────────────────────────────────────────────────────

fn strip_ansi(s: &str) -> String {
    // Matches SGR escape sequences: ESC [ ... m
    let re = regex::Regex::new(r"\x1b\[[0-9;]*m").expect("valid regex");
    re.replace_all(s, "").into_owned()
}

fn update_usage() -> Result<()> {
    // Dependency check — surface missing tool clearly.
    which("ccusage")?;

    eprintln!("fetching ccusage --json...");
    let out = Command::new("ccusage")
        .arg("--json")
        // Capture stderr separately; don't discard it on failure.
        .output()
        .context("failed to run ccusage")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("ccusage exited with {}: {stderr}", out.status);
    }

    let raw = String::from_utf8(out.stdout).context("ccusage output is not UTF-8")?;
    let stripped = strip_ansi(&raw);

    // Validate JSON before touching the output file.
    let value: serde_json::Value = serde_json::from_str(&stripped)
        .context("ccusage output is not valid JSON after ANSI strip")?;

    let out_path = workspace_root().join("examples/ccusage.json");
    let pretty = serde_json::to_vec_pretty(&value)?;
    atomic_write(&out_path, &pretty)?;
    eprintln!("wrote {}", out_path.display());
    Ok(())
}

// ── archive ───────────────────────────────────────────────────────────────────

const SSD_ROOT: &str = "/Volumes/Extreme SSD";
const SSD_VAULT: &str = "/Volumes/Extreme SSD/vault";

fn archive(project: &str) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let src = PathBuf::from(&home).join("dev").join(project);
    let ssd_dest = PathBuf::from(SSD_VAULT).join(project);
    let obsidian_index = PathBuf::from(&home).join("Documents/Obsidian Vault/archived-projects.md");

    // ── Pre-flight ────────────────────────────────────────────────────────────

    if !src.is_dir() {
        bail!("{} does not exist", src.display());
    }
    if !Path::new(SSD_ROOT).is_dir() {
        bail!("Extreme SSD is not mounted at {SSD_ROOT}");
    }
    if ssd_dest.exists() {
        bail!(
            "{} already exists — remove it first if you want to re-archive",
            ssd_dest.display()
        );
    }

    // ── Copy ─────────────────────────────────────────────────────────────────

    eprintln!("copying {} → {}", src.display(), ssd_dest.display());
    let status = Command::new("rsync")
        .args([
            "-rl",
            "--checksum",
            "--no-perms",
            "--no-owner",
            "--no-group",
            "--exclude=._*",
            "--exclude=.DS_Store",
        ])
        .arg(format!("{}/", src.display()))
        .arg(&ssd_dest)
        .status()
        .context("rsync not found")?;

    if !status.success() {
        bail!("rsync failed with {status}");
    }

    // ── Verify ────────────────────────────────────────────────────────────────

    eprintln!("verifying...");
    let src_hash = dir_hash(&src)?;
    let dst_hash = dir_hash(&ssd_dest)?;

    if src_hash != dst_hash {
        // Remove the failed copy — local source is untouched.
        std::fs::remove_dir_all(&ssd_dest).ok();
        bail!("checksum mismatch — aborting, local copy untouched");
    }
    eprintln!("verified ok ({src_hash})");

    // ── Index in Obsidian ─────────────────────────────────────────────────────

    let archive_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let gh_desc = gh_description(project).unwrap_or_default();

    if !obsidian_index.exists() {
        let header = "# Archived Projects\n\nProjects removed from the machine and stored on \
            the Extreme SSD under `Macbook/`.\n\n\
            | project | archived | ssd path | description |\n\
            |---------|----------|----------|-------------|\n";
        atomic_write(&obsidian_index, header.as_bytes())?;
    }

    let existing = std::fs::read_to_string(&obsidian_index)?;
    let row_marker = format!("| {project} |");
    if !existing.contains(&row_marker) {
        let row = format!(
            "| {project} | {archive_date} | Macbook/{project} | {} |\n",
            if gh_desc.is_empty() {
                "—".to_string()
            } else {
                gh_desc
            }
        );
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&obsidian_index)?;
        std::io::Write::write_all(&mut f, row.as_bytes())?;
        eprintln!("indexed in {}", obsidian_index.display());
    }

    // ── Remove local ──────────────────────────────────────────────────────────

    eprintln!("removing {}", src.display());
    std::fs::remove_dir_all(&src)?;
    eprintln!("done — {project} archived to SSD and removed from ~/dev");
    Ok(())
}

/// Compute a stable hash of all non-junk files under `dir`.
///
/// Mirrors the original script's approach: SHA-256 of each file's content,
/// collect hex strings, sort, then SHA-256 the joined sorted list.
/// Excludes `.git/`, `._*`, and `.DS_Store` to match rsync's `--exclude` list.
fn dir_hash(dir: &Path) -> Result<String> {
    let mut file_hashes: Vec<String> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            if !e.file_type().is_file() {
                return false;
            }
            let path = e.path();
            // Exclude .git contents.
            if path.components().any(|c| c.as_os_str() == ".git") {
                return false;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Exclude macOS resource forks and DS_Store.
            !name.starts_with("._") && name != ".DS_Store"
        })
        .map(|e| {
            let mut hasher = Sha256::new();
            let mut f = std::fs::File::open(e.path())
                .with_context(|| format!("opening {}", e.path().display()))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            hasher.update(&buf);
            Ok(hex::encode(hasher.finalize()))
        })
        .collect::<Result<Vec<_>>>()?;

    file_hashes.sort();
    let mut outer = Sha256::new();
    outer.update(file_hashes.join("\n").as_bytes());
    Ok(hex::encode(outer.finalize()))
}

/// Fetch a GitHub repo description via the `gh` CLI; returns empty string on any failure.
fn gh_description(project: &str) -> Result<String> {
    let out = Command::new("gh")
        .args([
            "repo",
            "view",
            &format!("{OWNER}/{project}"),
            "--json",
            "description",
            "--jq",
            ".description",
        ])
        .output()?;
    let desc = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(desc)
}

/// Return `Ok(())` if `name` is on PATH, or a clear error.
fn which(name: &str) -> Result<()> {
    let out = Command::new("which").arg(name).output();
    match out {
        Ok(o) if o.status.success() => Ok(()),
        _ => bail!("`{name}` not found on PATH — install it before running this task"),
    }
}
