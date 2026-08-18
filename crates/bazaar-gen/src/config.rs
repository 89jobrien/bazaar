use anyhow::Result;
use obfsck::{ObfuscationLevel, obfuscate_text};
use serde::Deserialize;
use std::fmt;
use std::path::Path;

#[derive(Clone)]
pub struct Config {
    pub github_token: Option<String>,
    pub github_user: String,
    pub crates_io_user: String,
    pub pypi_packages: Vec<String>,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_token = self
            .github_token
            .as_deref()
            .map(|t| obfuscate_text(t, ObfuscationLevel::Paranoid).0);
        f.debug_struct("Config")
            .field("github_token", &redacted_token)
            .field("github_user", &self.github_user)
            .field("crates_io_user", &self.crates_io_user)
            .field("pypi_packages", &self.pypi_packages)
            .finish()
    }
}

#[derive(Deserialize)]
struct PypiToml {
    packages: Vec<String>,
}

impl Config {
    pub fn from_env(pypi_toml_path: &Path) -> Result<Self> {
        let github_user = std::env::var("BAZAAR_GITHUB_USER")
            .map_err(|_| anyhow::anyhow!("BAZAAR_GITHUB_USER env var is required"))?;
        let crates_io_user = std::env::var("BAZAAR_CRATES_IO_USER")
            .map_err(|_| anyhow::anyhow!("BAZAAR_CRATES_IO_USER env var is required"))?;
        let github_token = std::env::var("GITHUB_TOKEN").ok();

        if github_token.is_none() {
            eprintln!(
                "warning: GITHUB_TOKEN not set — using unauthenticated GitHub API (60 req/hr)"
            );
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
        unsafe {
            std::env::remove_var("BAZAAR_GITHUB_USER");
            std::env::remove_var("BAZAAR_CRATES_IO_USER");
        }
        let result = Config::from_env(Path::new("nonexistent.toml"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("BAZAAR_GITHUB_USER")
        );
    }

    #[test]
    #[serial_test::serial]
    fn missing_crates_io_user_returns_error() {
        unsafe {
            std::env::remove_var("BAZAAR_GITHUB_USER");
            std::env::remove_var("BAZAAR_CRATES_IO_USER");
            std::env::set_var("BAZAAR_GITHUB_USER", "testuser");
        }
        let result = Config::from_env(Path::new("nonexistent.toml"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("BAZAAR_CRATES_IO_USER")
        );
        unsafe {
            std::env::remove_var("BAZAAR_GITHUB_USER");
        }
    }

    #[test]
    #[serial_test::serial]
    fn parses_pypi_toml() {
        unsafe {
            std::env::remove_var("BAZAAR_GITHUB_USER");
            std::env::remove_var("BAZAAR_CRATES_IO_USER");
        }
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"packages = ["foo", "bar"]"#).unwrap();
        unsafe {
            std::env::set_var("BAZAAR_GITHUB_USER", "u");
            std::env::set_var("BAZAAR_CRATES_IO_USER", "u");
        }
        let cfg = Config::from_env(f.path()).unwrap();
        assert_eq!(cfg.pypi_packages, vec!["foo", "bar"]);
        unsafe {
            std::env::remove_var("BAZAAR_GITHUB_USER");
            std::env::remove_var("BAZAAR_CRATES_IO_USER");
        }
    }

    #[test]
    fn debug_redacts_github_token() {
        // Built at runtime (not a literal) so this fixture isn't itself
        // flagged as a leaked secret by pre-commit scanning.
        let fake_token = format!("{}_{}", "ghp", "a".repeat(40));
        let cfg = Config {
            github_token: Some(fake_token.clone()),
            github_user: "someuser".to_string(),
            crates_io_user: "someuser".to_string(),
            pypi_packages: vec![],
        };
        let debug_output = format!("{cfg:?}");
        assert!(!debug_output.contains(&fake_token));
        assert!(debug_output.contains("someuser"));
    }
}
