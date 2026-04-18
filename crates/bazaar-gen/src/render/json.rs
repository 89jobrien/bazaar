use crate::model::Project;
use anyhow::Result;

pub fn render_data_json(projects: &[Project]) -> Result<String> {
    serde_json::to_string_pretty(projects).map_err(|e| anyhow::anyhow!("json render failed: {e}"))
}
