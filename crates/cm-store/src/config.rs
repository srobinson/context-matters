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
use cm_core::ScopeInferenceStrategy;
use serde::Deserialize;

use crate::project::{default_base_dir, resolve_home_dir};

/// Runtime configuration for the context-matters store.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory containing `cm.db` and optional `.cm.config.toml`.
    pub data_dir: PathBuf,
    /// Tracing filter level (e.g. `"info"`, `"debug"`).
    pub log_level: String,
    /// Strategy used for `cwd_inferred` scope resolution.
    pub scope_inference_strategy: ScopeInferenceStrategy,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = default_base_dir().unwrap_or_else(|_| PathBuf::from("~/.context-matters"));
        Self {
            data_dir,
            log_level: "warn".to_owned(),
            scope_inference_strategy: ScopeInferenceStrategy::Filesystem,
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
pub use cm_core::CM_CONFIG_FILENAME as CONFIG_FILENAME;

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

# User-level scope inference strategy for `cwd_inferred` selectors.
# `filesystem` preserves git and cwd based inference. `custom` disables
# cwd_inferred and requires explicit scope input. `k8s` is reserved.
# Read from $CM_DATA_DIR/{filename} or ~/.context-matters/{filename};
# project-local config files cannot override it.
# [scope_inference]
# strategy = "filesystem"

# Recall ranking mode. `legacy` preserves existing scope-depth ordering.
# `shadow` is reserved for observe-only diffing. `live` serves the
# deterministic kind/confidence/priority rank key.
# Override with CM_RECALL_RANKING env var.
# [recall]
# ranking_mode = "legacy"
"#,
        filename = CONFIG_FILENAME
    )
}

/// Intermediate struct for deserializing the TOML config file.
/// Fields are all optional because the file itself is optional and
/// any missing field falls back to the default.
///
/// `deny_unknown_fields` treats unknown keys as deserialization errors,
/// causing `find_and_parse_config` to warn and fall back to defaults.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    data_dir: Option<String>,
    log_level: Option<String>,
    scope_inference: Option<FileScopeInferenceConfig>,
    /// Parsed so shared config files with `[recall]` remain valid.
    /// cm-capabilities owns the ranking mode semantics.
    #[serde(rename = "recall")]
    _recall: Option<FileRecallConfig>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileScopeInferenceConfig {
    strategy: Option<ScopeInferenceStrategy>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileRecallConfig {
    #[serde(rename = "ranking_mode")]
    _ranking_mode: Option<String>,
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

    // Scope inference is intentionally user-level only. It never comes from
    // $CWD/.cm.config.toml because the MCP server runs as one process per user.
    if let Some(strategy) = find_and_parse_user_config()
        .and_then(|file_cfg| file_cfg.scope_inference)
        .and_then(|scope_inference| scope_inference.strategy)
    {
        config.scope_inference_strategy = strategy;
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
///
/// "First found wins": when a file exists at a higher-precedence path but
/// fails to read or parse, we warn and fall back to defaults rather than
/// trying lower-precedence paths. This prevents a broken project-local
/// config from being silently overridden by a valid global one.
fn find_and_parse_config() -> Option<FileConfig> {
    find_first_config(config_search_paths())
}

fn find_and_parse_user_config() -> Option<FileConfig> {
    find_first_config(user_config_search_paths())
}

fn find_first_config(candidates: Vec<PathBuf>) -> Option<FileConfig> {
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
                        return None;
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to read config file, using defaults"
                    );
                    return None;
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

    paths.extend(user_config_search_paths());

    paths
}

fn user_config_search_paths() -> Vec<PathBuf> {
    let filename = ".cm.config.toml";
    let mut paths = Vec::with_capacity(2);

    // 1. $CM_DATA_DIR/.cm.config.toml (tilde-expanded for consistency)
    if let Ok(dir) = std::env::var("CM_DATA_DIR")
        && let Ok(expanded) = expand_tilde(&dir)
    {
        paths.push(expanded.join(filename));
    }

    // 2. ~/.context-matters/.cm.config.toml
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
    if path == "~" {
        resolve_home_dir()
    } else if let Some(rest) = path.strip_prefix("~/") {
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
        assert_eq!(config.log_level, "warn");
        assert_eq!(
            config.scope_inference_strategy,
            ScopeInferenceStrategy::Filesystem
        );
    }

    #[test]
    fn db_path_appends_cm_db() {
        let config = Config {
            data_dir: PathBuf::from("/tmp/test-cm"),
            log_level: "debug".to_owned(),
            scope_inference_strategy: ScopeInferenceStrategy::Filesystem,
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
    fn expand_tilde_bare_resolves_to_home() {
        let expanded = expand_tilde("~").unwrap();
        assert!(expanded.is_absolute());
        assert!(!expanded.to_string_lossy().contains('~'));
    }

    #[test]
    fn expand_tilde_leaves_absolute_paths() {
        let expanded = expand_tilde("/absolute/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn parse_rejects_unknown_keys() {
        let result = toml::from_str::<FileConfig>(
            r#"
            data_dir = "/tmp"
            unknown_key = "value"
            "#,
        );
        assert!(result.is_err(), "unknown keys should be rejected");
    }

    #[test]
    fn parse_empty_toml_yields_defaults() {
        let cfg: FileConfig = toml::from_str("").unwrap();
        assert!(cfg.data_dir.is_none());
        assert!(cfg.log_level.is_none());
        assert!(cfg.scope_inference.is_none());
        assert!(cfg._recall.is_none());
    }

    #[test]
    fn parse_full_toml() {
        let cfg: FileConfig = toml::from_str(
            r#"
            data_dir = "~/custom-dir"
            log_level = "debug"

            [scope_inference]
            strategy = "custom"

            [recall]
            ranking_mode = "live"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.data_dir.as_deref(), Some("~/custom-dir"));
        assert_eq!(cfg.log_level.as_deref(), Some("debug"));
        assert_eq!(
            cfg.scope_inference.unwrap().strategy,
            Some(ScopeInferenceStrategy::Custom)
        );
        assert_eq!(cfg._recall.unwrap()._ranking_mode.as_deref(), Some("live"));
    }

    #[test]
    fn parse_partial_scope_inference_toml_uses_default_strategy() {
        let cfg: FileConfig = toml::from_str(
            r#"
            [scope_inference]
            "#,
        )
        .unwrap();
        assert!(cfg.scope_inference.unwrap().strategy.is_none());
    }

    #[test]
    fn validate_rejects_empty_data_dir() {
        let config = Config {
            data_dir: PathBuf::new(),
            log_level: "warn".to_owned(),
            scope_inference_strategy: ScopeInferenceStrategy::Filesystem,
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.to_string().contains("must not be empty"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_rejects_relative_data_dir() {
        let config = Config {
            data_dir: PathBuf::from("relative/path"),
            log_level: "warn".to_owned(),
            scope_inference_strategy: ScopeInferenceStrategy::Filesystem,
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.to_string().contains("must be an absolute path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_accepts_absolute_data_dir() {
        let config = Config {
            data_dir: PathBuf::from("/tmp/cm-test"),
            log_level: "warn".to_owned(),
            scope_inference_strategy: ScopeInferenceStrategy::Filesystem,
        };
        config.validate().unwrap();
    }

    #[test]
    fn load_propagates_validation_error_for_relative_data_dir() {
        // Set CM_DATA_DIR to a relative path; load() should propagate the validation error
        temp_env::with_vars(
            [
                ("CM_DATA_DIR", Some("relative/path")),
                ("CM_LOG_LEVEL", None::<&str>),
            ],
            || {
                let err = load().unwrap_err();
                assert!(
                    err.to_string().contains("must be an absolute path"),
                    "unexpected error: {err}"
                );
            },
        );
    }

    #[test]
    fn load_propagates_validation_error_for_empty_data_dir() {
        temp_env::with_vars(
            [("CM_DATA_DIR", Some("")), ("CM_LOG_LEVEL", None::<&str>)],
            || {
                let err = load().unwrap_err();
                assert!(
                    err.to_string().contains("must not be empty"),
                    "unexpected error: {err}"
                );
            },
        );
    }

    #[test]
    fn load_succeeds_with_absolute_cm_data_dir() {
        temp_env::with_vars(
            [
                ("CM_DATA_DIR", Some("/tmp/cm-test-load")),
                ("CM_LOG_LEVEL", None::<&str>),
            ],
            || {
                let config = load().unwrap();
                assert_eq!(config.data_dir, PathBuf::from("/tmp/cm-test-load"));
            },
        );
    }

    #[test]
    fn load_expands_tilde_in_cm_data_dir() {
        temp_env::with_vars(
            [
                ("CM_DATA_DIR", Some("~/custom-cm")),
                ("CM_LOG_LEVEL", None::<&str>),
            ],
            || {
                let config = load().unwrap();
                assert!(config.data_dir.is_absolute());
                assert!(config.data_dir.ends_with("custom-cm"));
                assert!(!config.data_dir.to_string_lossy().contains('~'));
            },
        );
    }

    #[test]
    fn load_respects_cm_log_level() {
        temp_env::with_vars(
            [
                ("CM_DATA_DIR", Some("/tmp/cm-test-log")),
                ("CM_LOG_LEVEL", Some("trace")),
            ],
            || {
                let config = load().unwrap();
                assert_eq!(config.log_level, "trace");
            },
        );
    }

    #[test]
    fn load_respects_user_scope_inference_strategy() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILENAME),
            r#"
            [scope_inference]
            strategy = "custom"
            "#,
        )
        .unwrap();
        let data_dir = dir.path().to_string_lossy().into_owned();

        temp_env::with_vars(
            [
                ("CM_DATA_DIR", Some(data_dir.as_str())),
                ("CM_LOG_LEVEL", None::<&str>),
            ],
            || {
                let config = load().unwrap();
                assert_eq!(
                    config.scope_inference_strategy,
                    ScopeInferenceStrategy::Custom
                );
            },
        );
    }

    #[test]
    fn user_scope_inference_search_excludes_current_dir() {
        temp_env::with_vars(
            [
                ("CM_DATA_DIR", None::<&str>),
                ("CM_LOG_LEVEL", None::<&str>),
            ],
            || {
                let cwd_config = std::env::current_dir().unwrap().join(CONFIG_FILENAME);
                assert!(
                    !user_config_search_paths()
                        .iter()
                        .any(|path| path == &cwd_config),
                    "user-level scope inference must not search project-local config"
                );
            },
        );
    }

    #[test]
    fn config_template_is_valid_toml_when_values_uncommented() {
        let template = config_template();
        // Uncomment only lines that look like TOML key = value pairs
        let uncommented: String = template
            .lines()
            .filter_map(|line| {
                if let Some(stripped) = line.strip_prefix("# ")
                    && (stripped.contains(" = ") || stripped.starts_with('['))
                {
                    return Some(stripped);
                }
                if !line.starts_with('#') && !line.is_empty() {
                    return Some(line);
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n");
        let result: Result<FileConfig, _> = toml::from_str(&uncommented);
        assert!(result.is_ok(), "template is not valid TOML: {result:?}");
    }
}
