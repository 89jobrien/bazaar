use anyhow::{Context, Result};
use bazaar_types::insights::Insights;
use std::path::Path;

pub fn load_insights(path: &Path) -> Result<Option<Insights>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read insights from {}", path.display()))?;
    let insights: Insights = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse insights YAML from {}", path.display()))?;
    Ok(Some(insights))
}
