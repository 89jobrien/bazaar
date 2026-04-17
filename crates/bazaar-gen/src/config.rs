use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub github_token: Option<String>,
    pub github_user: String,
    pub crates_io_user: String,
    pub pypi_packages: Vec<String>,
    pub plugin_manifest: std::path::PathBuf,
}

#[derive(Deserialize)]
struct PypiToml {
    packages: Vec<String>,
}

impl Config {
    pub fn from_env(pypi_toml_path: &Path, plugin_manifest: std::path::PathBuf) -> Result<Self> {
        let github_user = std::env::var("BAZAAR_GITHUB_USER")
            .map_err(|_| anyhow::anyhow!("BAZAAR_GITHUB_USER env var is required"))?;
        let crates_io_user = std::env::var("BAZAAR_CRATES_IO_USER")
            .map_err(|_| anyhow::anyhow!("BAZAAR_CRATES_IO_USER env var is required"))?;
        let github_token = std::env::var("GITHUB_TOKEN").ok();

        if github_token.is_none() {
            eprintln!("warning: GITHUB_TOKEN not set — using unauthenticated GitHub API (60 req/hr)");
        }

        let pypi_packages = if pypi_toml_path.exists() {
            let raw = std::fs::read_to_string(pypi_toml_path)?;
            let parsed: PypiToml = toml::from_str(&raw)?;
            parsed.packages
        } else {
            vec![]
        };

        Ok(Config {
            github_token,
            github_user,
            crates_io_user,
            pypi_packages,
            plugin_manifest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    #[serial_test::serial]
    fn missing_github_user_returns_error() {
        std::env::remove_var("BAZAAR_GITHUB_USER");
        std::env::remove_var("BAZAAR_CRATES_IO_USER");
        let result = Config::from_env(Path::new("nonexistent.toml"), "manifest.json".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BAZAAR_GITHUB_USER"));
    }

    #[test]
    #[serial_test::serial]
    fn missing_crates_io_user_returns_error() {
        std::env::remove_var("BAZAAR_GITHUB_USER");
        std::env::remove_var("BAZAAR_CRATES_IO_USER");
        std::env::set_var("BAZAAR_GITHUB_USER", "testuser");
        let result = Config::from_env(Path::new("nonexistent.toml"), "manifest.json".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BAZAAR_CRATES_IO_USER"));
        std::env::remove_var("BAZAAR_GITHUB_USER");
    }

    #[test]
    #[serial_test::serial]
    fn parses_pypi_toml() {
        std::env::remove_var("BAZAAR_GITHUB_USER");
        std::env::remove_var("BAZAAR_CRATES_IO_USER");
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"packages = ["foo", "bar"]"#).unwrap();
        std::env::set_var("BAZAAR_GITHUB_USER", "u");
        std::env::set_var("BAZAAR_CRATES_IO_USER", "u");
        let cfg = Config::from_env(f.path(), "manifest.json".into()).unwrap();
        assert_eq!(cfg.pypi_packages, vec!["foo", "bar"]);
        std::env::remove_var("BAZAAR_GITHUB_USER");
        std::env::remove_var("BAZAAR_CRATES_IO_USER");
    }
}

