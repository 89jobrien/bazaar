use crate::model::Project;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct ProjectOverride {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct HeaderConfig {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_subtitle")]
    pub subtitle: String,
    #[serde(default)]
    pub overrides: HashMap<String, ProjectOverride>,
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub tags: HashMap<String, Vec<String>>,
}

fn default_title() -> String {
    "Open Source".to_string()
}

fn default_subtitle() -> String {
    "Projects I maintain or contribute to.".to_string()
}

impl HeaderConfig {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let cfg: HeaderConfig = serde_yaml::from_str(&text)?;
        Ok(cfg)
    }

    /// Apply overrides, pinned order, and tags to the project list.
    pub fn apply(&self, mut projects: Vec<Project>) -> Vec<Project> {
        // Apply description overrides
        for p in &mut projects {
            if let Some(ov) = self.overrides.get(&p.name)
                && let Some(desc) = &ov.description
            {
                p.description = Some(desc.clone());
            }
        }

        // Build reverse tag map: project name -> tags
        let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();
        for (tag, names) in &self.tags {
            for name in names {
                tag_map.entry(name.clone()).or_default().push(tag.clone());
            }
        }
        for p in &mut projects {
            if let Some(tags) = tag_map.get(&p.name) {
                p.tags = tags.clone();
                p.tags.sort();
            }
        }

        // Apply pinned order: move pinned projects to front, preserving pinned order
        if self.pinned.is_empty() {
            return projects;
        }

        let mut pinned_items: Vec<Project> = Vec::with_capacity(self.pinned.len());
        let mut rest: Vec<Project> = Vec::new();

        // Preserve pinned order
        for pin in &self.pinned {
            if let Some(pos) = projects.iter().position(|p| &p.name == pin) {
                pinned_items.push(projects.remove(pos));
            }
        }
        rest.extend(projects);

        pinned_items.extend(rest);
        pinned_items
    }
}
