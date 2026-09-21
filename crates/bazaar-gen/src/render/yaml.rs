//! Serializes showcase projects as YAML.

use crate::model::Project;
use anyhow::Result;

/// Serializes projects as YAML.
pub fn render_data_yaml(projects: &[Project]) -> Result<String> {
    serde_yaml::to_string(projects).map_err(|e| anyhow::anyhow!("yaml render failed: {e}"))
}
