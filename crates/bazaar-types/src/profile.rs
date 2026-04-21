use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileProject {
    pub name: String,
    pub description: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStats {
    pub sessions_per_day: String,
    pub total_sessions_march_april_2026: u32,
    pub commit_streak_peak: String,
    pub spec_to_ship_best: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLinks {
    pub github: String,
    pub crates_io: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub handle: String,
    pub location: String,
    pub role: String,
    pub tagline: String,
    pub summary: String,
    pub focus_areas: Vec<String>,
    pub active_projects: Vec<ProfileProject>,
    pub workflow_style: String,
    pub stats: ProfileStats,
    pub links: ProfileLinks,
}
