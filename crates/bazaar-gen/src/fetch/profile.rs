use crate::model::Profile;
use anyhow::{Context, Result};
use std::path::Path;

pub fn load_profile(path: &Path) -> Result<Profile> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read profile from {}", path.display()))?;
    serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse profile YAML from {}", path.display()))
}
