use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct ContentsMeta {
    sha: String,
}

/// Push `content` to `path` in `repo` (e.g. "owner/repo") via the GitHub Contents API.
pub async fn push_to_pages(
    client: &Client,
    token: &str,
    repo: &str,
    path: &str,
    content: &[u8],
    commit_message: &str,
) -> Result<()> {
    let url = format!("https://api.github.com/repos/{}/contents/{}", repo, path);

    // Fetch existing SHA if the file already exists (required for updates).
    let existing_sha: Option<String> = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "bazaar-gen/bz")
        .send()
        .await?
        .json::<ContentsMeta>()
        .await
        .ok()
        .map(|m| m.sha);

    let encoded = STANDARD.encode(content);

    let mut body = serde_json::json!({
        "message": commit_message,
        "content": encoded,
    });
    if let Some(sha) = existing_sha {
        body["sha"] = serde_json::Value::String(sha);
    }

    let resp = client
        .put(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "bazaar-gen/bz")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("pages push failed ({}): {}", status, text);
    }

    Ok(())
}
