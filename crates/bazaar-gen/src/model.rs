use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    GitHubRepo,
    CratesIo,
    PyPI,
    ClaudePlugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub message: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub kinds: Vec<Kind>,
    pub language: Option<String>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
    pub stars: Option<u32>,
    pub downloads: Option<u64>,
    pub recent_commits: Vec<Commit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Project {
    pub fn slug(&self) -> String {
        self.name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
            .trim_matches('-')
            .to_string()
    }
}

/// Merge projects from multiple sources. Projects with the same `name` are combined:
/// kinds are unioned, and fields from later entries fill in None fields from earlier ones.
pub fn merge(mut projects: Vec<Project>) -> Vec<Project> {
    let mut map: indexmap::IndexMap<String, Project> = indexmap::IndexMap::new();
    for p in projects.drain(..) {
        map.entry(p.name.clone())
            .and_modify(|existing| {
                for k in &p.kinds {
                    if !existing.kinds.contains(k) {
                        existing.kinds.push(k.clone());
                    }
                }
                if existing.description.is_none() { existing.description = p.description.clone(); }
                if existing.pushed_at.is_none() { existing.pushed_at = p.pushed_at; }
                if existing.version.is_none() { existing.version = p.version.clone(); }
                if existing.stars.is_none() { existing.stars = p.stars; }
                if existing.downloads.is_none() { existing.downloads = p.downloads; }
                if existing.recent_commits.is_empty() { existing.recent_commits = p.recent_commits.clone(); }
            })
            .or_insert(p);
    }
    let mut out: Vec<Project> = map.into_values().collect();
    // Sort by pushed_at descending, None last
    out.sort_by(|a, b| match (a.pushed_at, b.pushed_at) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make(name: &str, kind: Kind, pushed: Option<DateTime<Utc>>) -> Project {
        Project {
            name: name.to_string(),
            description: None,
            url: format!("https://example.com/{name}"),
            kinds: vec![kind],
            language: None,
            pushed_at: pushed,
            version: None,
            stars: None,
            downloads: None,
            recent_commits: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn merge_combines_same_name() {
        let a = make("foo", Kind::GitHubRepo, None);
        let b = make("foo", Kind::CratesIo, None);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert!(out[0].kinds.contains(&Kind::GitHubRepo));
        assert!(out[0].kinds.contains(&Kind::CratesIo));
    }

    #[test]
    fn merge_keeps_distinct_names() {
        let a = make("foo", Kind::GitHubRepo, None);
        let b = make("bar", Kind::CratesIo, None);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn sort_pushed_at_descending() {
        let older = make("old", Kind::GitHubRepo, Some(Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()));
        let newer = make("new", Kind::GitHubRepo, Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()));
        let out = merge(vec![older, newer]);
        assert_eq!(out[0].name, "new");
        assert_eq!(out[1].name, "old");
    }

    #[test]
    fn none_pushed_at_sorts_last() {
        let with_date = make("dated", Kind::GitHubRepo, Some(Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap()));
        let without = make("nodate", Kind::CratesIo, None);
        let out = merge(vec![without, with_date]);
        assert_eq!(out[0].name, "dated");
        assert_eq!(out[1].name, "nodate");
    }
}
