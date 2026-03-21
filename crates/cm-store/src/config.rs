//! Configuration loading for context-matters.
//!
//! Precedence: environment variables > TOML config file > defaults.
//!
//! Config file resolution (first found wins):
//! 1. `$CWD/.cm.config.toml`
//! 2. `$CM_DATA_DIR/.cm.config.toml` (if env var is set)
//! 3. `~/.context-matters/.cm.config.toml`

use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use crate::project::{default_base_dir, resolve_home_dir};

/// Runtime configuration for the context-matters store.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory containing `cm.db` and optional `.cm.config.toml`.
    pub data_dir: PathBuf,
    /// Tracing filter level (e.g. `"info"`, `"debug"`).
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = default_base_dir().unwrap_or_else(|_| PathBuf::from("~/.context-matters"));
        Self {
            data_dir,
            log_level: "info".to_owned(),
        }
    }
}

impl Config {
    /// Full path to the SQLite database file.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("cm.db")
    }

    /// Validate the resolved configuration. Returns an error if any
    /// semantic rule is violated (fail closed on invalid config).
    pub fn validate(&self) -> Result<()> {
        if self.data_dir.as_os_str().is_empty() {
            anyhow::bail!("data_dir must not be empty");
        }
        if !self.data_dir.is_absolute() {
            anyhow::bail!(
                "data_dir must be an absolute path after tilde expansion, got: {:?}",
                self.data_dir
            );
        }
        Ok(())
    }
}

/// Config file name used for both loading and generating.
pub const CONFIG_FILENAME: &str = ".cm.config.toml";

/// Returns a commented TOML config template with all options and defaults.
#[must_use]
pub fn config_template() -> String {
    format!(
        r#"# context-matters configuration
#
# Config file resolution (first found wins):
#   1. $CWD/{filename}   (project-local)
#   2. $CM_DATA_DIR/{filename}  (custom data directory)
#   3. ~/.context-matters/{filename}  (global fallback)
#
# Environment variables override all file settings:
#   CM_DATA_DIR, CM_LOG_LEVEL

# Directory where the database and state files are stored.
# Override with CM_DATA_DIR env var.
# data_dir = "~/.context-matters"

# Tracing filter level: "warn", "info", "debug", "trace".
# Override with CM_LOG_LEVEL env var, or use RUST_LOG for fine-grained control.
# log_level = "warn"
"#,
        filename = CONFIG_FILENAME
    )
}

/// Intermediate struct for deserializing the TOML config file.
/// Fields are all optional because the file itself is optional and
/// any missing field falls back to the default.
#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    data_dir: Option<String>,
    log_level: Option<String>,
}

/// Load configuration with precedence: env vars > TOML file > defaults.
///
/// After merging all three layers, calls `validate()` to reject
/// semantically invalid resolved config (empty or relative data_dir).
pub fn load() -> Result<Config> {
    let mut config = Config::default();

    // Layer 1: TOML file (lowest precedence after defaults)
    if let Some(file_cfg) = find_and_parse_config() {
        if let Some(dir) = file_cfg.data_dir {
            config.data_dir = expand_tilde(&dir)?;
        }
        if let Some(level) = file_cfg.log_level {
            config.log_level = level;
        }
    }

    // Layer 2: Environment variables (highest precedence)
    if let Ok(dir) = std::env::var("CM_DATA_DIR") {
        config.data_dir = expand_tilde(&dir)?;
    }
    if let Ok(level) = std::env::var("CM_LOG_LEVEL") {
        config.log_level = level;
    }

    config.validate()?;
    Ok(config)
}

/// Search for a config file in resolution order, parse the first found.
fn find_and_parse_config() -> Option<FileConfig> {
    let candidates = config_search_paths();
    for path in candidates {
        if path.exists() {
            tracing::debug!(path = %path.display(), "loading config file");
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<FileConfig>(&contents) {
                    Ok(cfg) => return Some(cfg),
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "failed to parse config file, using defaults"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to read config file, using defaults"
                    );
                }
            }
        }
    }
    None
}

/// Returns the ordered list of config file paths to search.
fn config_search_paths() -> Vec<PathBuf> {
    let filename = ".cm.config.toml";
    let mut paths = Vec::with_capacity(3);

    // 1. $CWD/.cm.config.toml
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(filename));
    }

    // 2. $CM_DATA_DIR/.cm.config.toml
    if let Ok(dir) = std::env::var("CM_DATA_DIR") {
        paths.push(PathBuf::from(dir).join(filename));
    }

    // 3. ~/.context-matters/.cm.config.toml
    if let Ok(base) = default_base_dir() {
        paths.push(base.join(filename));
    }

    paths
}

/// Expand a leading `~/` to the user's home directory.
///
/// Absolute paths pass through unchanged. Returns an error if the
/// home directory cannot be resolved when tilde expansion is needed.
fn expand_tilde(path: &str) -> Result<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        Ok(resolve_home_dir()?.join(rest))
    } else {
        Ok(PathBuf::from(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_context_matters_dir() {
        let config = Config::default();
        assert!(config.data_dir.ends_with(".context-matters"));
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn db_path_appends_cm_db() {
        let config = Config {
            data_dir: PathBuf::from("/tmp/test-cm"),
            log_level: "debug".to_owned(),
        };
        assert_eq!(config.db_path(), PathBuf::from("/tmp/test-cm/cm.db"));
    }

    #[test]
    fn expand_tilde_replaces_home() {
        let expanded = expand_tilde("~/foo/bar").unwrap();
        // Should not start with ~ anymore
        assert!(!expanded.to_string_lossy().starts_with('~'));
        assert!(expanded.to_string_lossy().ends_with("foo/bar"));
    }

    #[test]
    fn expand_tilde_leaves_absolute_paths() {
        let expanded = expand_tilde("/absolute/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn parse_empty_toml_yields_defaults() {
        let cfg: FileConfig = toml::from_str("").unwrap();
        assert!(cfg.data_dir.is_none());
        assert!(cfg.log_level.is_none());
    }

    #[test]
    fn parse_full_toml() {
        let cfg: FileConfig = toml::from_str(
            r#"
            data_dir = "~/custom-dir"
            log_level = "debug"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.data_dir.as_deref(), Some("~/custom-dir"));
        assert_eq!(cfg.log_level.as_deref(), Some("debug"));
    }
}
