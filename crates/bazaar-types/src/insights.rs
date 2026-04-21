use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InsightsProject {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InsightsStats {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions_per_day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_sessions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_added: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_removed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_to_ship_best: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InsightsArea {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions: Option<u32>,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Insights {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_areas: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_projects: Vec<InsightsProject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<InsightsStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_style: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub what_you_work_on: Vec<InsightsArea>,
}
