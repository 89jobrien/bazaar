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

pub fn deploy(site_dir: &Path, repo: &str, token: Option<&str>) -> Result<()> {
    let tmp = tempfile::tempdir().context("create tempdir")?;
    let clone_dir = tmp.path().join("site");

    // Build authenticated URL if token provided
    let url = if let Some(t) = token {
        format!("https://x-access-token:{t}@github.com/{repo}.git")
    } else {
        format!("https://github.com/{repo}.git")
    };

    eprintln!("cloning {repo}...");
    git(tmp.path(), &["clone", "--depth=1", &url, "site"])?;

    // Copy generated files into the clone, preserving .git
    eprintln!("syncing files...");
    for entry in std::fs::read_dir(site_dir).context("read site dir")? {
        let entry = entry?;
        let dest = clone_dir.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }

    git(&clone_dir, &["config", "user.name", "bz"])?;
    git(&clone_dir, &["config", "user.email", "bz@users.noreply.github.com"])?;
    git(&clone_dir, &["add", "-A"])?;

    // Check if there's anything to commit
    let output = Command::new("git")
        .args(["diff", "--staged", "--quiet"])
        .current_dir(&clone_dir)
        .status()?;
    if output.success() {
        eprintln!("no changes to deploy");
        return Ok(());
    }

    git(&clone_dir, &["commit", "-m", "chore: sync showcase from bazaar"])?;
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
