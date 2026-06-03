use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("git {} failed: {}", args.join(" "), status);
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
