use anyhow::{Context, Result};
use obfsck::{ObfuscationLevel, obfuscate_text};
use std::path::Path;
use std::process::Command;

/// Redact any embedded secrets (e.g. a GitHub token in a clone URL) before
/// the command line reaches an error message, log line, or context string.
fn redact_args(args: &[&str]) -> String {
    obfuscate_text(&args.join(" "), ObfuscationLevel::Paranoid).0
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("git {}", redact_args(args)))?;
    if !status.success() {
        anyhow::bail!("git {} failed: {}", redact_args(args), status);
    }
    Ok(())
}

fn repo_url(repo: &str, token: Option<&str>) -> String {
    if let Some(t) = token {
        format!("https://x-access-token:{t}@github.com/{repo}.git")
    } else {
        format!("https://github.com/{repo}.git")
    }
}

fn sync_site_files(site_dir: &Path, clone_dir: &std::path::Path) -> Result<()> {
    for entry in std::fs::read_dir(site_dir).context("read site dir")? {
        let entry = entry?;
        let dest = clone_dir.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn has_staged_changes(clone_dir: &std::path::Path) -> Result<bool> {
    let status = Command::new("git")
        .args(["diff", "--staged", "--quiet"])
        .current_dir(clone_dir)
        .status()?;
    Ok(!status.success())
}

pub fn deploy(site_dir: &Path, repo: &str, token: Option<&str>) -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let clone_dir = tmp.path().join("site");
    let url = repo_url(repo, token);

    eprintln!("cloning {repo}...");
    git(tmp.path(), &["clone", "--depth=1", &url, "site"])?;

    eprintln!("syncing files...");
    sync_site_files(site_dir, &clone_dir)?;

    git(&clone_dir, &["config", "user.name", "bz"])?;
    git(
        &clone_dir,
        &["config", "user.email", "bz@users.noreply.github.com"],
    )?;
    git(&clone_dir, &["add", "-A"])?;

    if !has_staged_changes(&clone_dir)? {
        eprintln!("no changes to deploy");
        return Ok(());
    }

    git(
        &clone_dir,
        &["commit", "-m", "chore: sync showcase from bazaar"],
    )?;
    git(&clone_dir, &["push"])?;
    eprintln!("deployed to {repo}");
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_args_scrubs_token_embedded_in_clone_url() {
        // Built at runtime (not a literal) so this fixture isn't itself
        // flagged as a leaked secret by pre-commit scanning.
        let token = format!("{}_{}", "ghp", "c".repeat(40));
        let url = repo_url("89jobrien/89jobrien.github.io", Some(&token));
        let redacted = redact_args(&["clone", "--depth=1", &url, "site"]);
        assert!(!redacted.contains(&token));
        assert!(redacted.contains("clone"));
    }
}
