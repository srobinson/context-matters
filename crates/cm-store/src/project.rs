//! Data directory setup for context-matters.
//!
//! Resolves the default base directory (`~/.context-matters`) and ensures
//! it exists on first run. Env-var access is parameterized through the
//! private `HomeEnvironment` trait so unit tests can avoid mutating the
//! process-global `HOME` and `USERPROFILE` variables (the prior pattern
//! using `temp_env::with_vars` raced against parallel reads, see ALP-2503).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Test seam for reading home-related environment variables.
///
/// Private. Production uses `SystemHomeEnvironment`; tests construct a
/// fake. Same shape as `CwdEnvironment` in `cm-capabilities::scope::resolution`.
trait HomeEnvironment {
    fn var(&self, key: &str) -> Result<String, std::env::VarError>;
}

struct SystemHomeEnvironment;

impl HomeEnvironment for SystemHomeEnvironment {
    fn var(&self, key: &str) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }
}

/// Resolve the user's home directory from environment variables.
///
/// Reads `HOME` (Unix) or `USERPROFILE` (Windows). Returns an error if
/// neither is set, or if the value is empty or a relative path.
pub fn resolve_home_dir() -> Result<PathBuf> {
    resolve_home_dir_with_environment(&SystemHomeEnvironment)
}

fn resolve_home_dir_with_environment(env: &impl HomeEnvironment) -> Result<PathBuf> {
    let home = env
        .var("HOME")
        .or_else(|_| env.var("USERPROFILE"))
        .map_err(|_| {
            anyhow::anyhow!(
                "could not determine home directory: neither HOME nor USERPROFILE is set"
            )
        })?;

    let path = PathBuf::from(&home);
    if home.is_empty() || !path.is_absolute() {
        bail!("home directory must be an absolute path, got: {home:?}");
    }
    Ok(path)
}

/// Returns the default base directory: `$HOME/.context-matters`.
///
/// Returns an error if the home directory cannot be resolved.
pub fn default_base_dir() -> Result<PathBuf> {
    default_base_dir_with_environment(&SystemHomeEnvironment)
}

fn default_base_dir_with_environment(env: &impl HomeEnvironment) -> Result<PathBuf> {
    Ok(resolve_home_dir_with_environment(env)?.join(".context-matters"))
}

/// Creates the data directory (and parents) if it does not exist.
pub fn ensure_data_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create data directory: {}", dir.display()))?;
        tracing::info!(path = %dir.display(), "created data directory");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeHomeEnvironment {
        home: Option<String>,
        userprofile: Option<String>,
    }

    impl HomeEnvironment for FakeHomeEnvironment {
        fn var(&self, key: &str) -> Result<String, std::env::VarError> {
            let value = match key {
                "HOME" => self.home.as_deref(),
                "USERPROFILE" => self.userprofile.as_deref(),
                _ => return Err(std::env::VarError::NotPresent),
            };
            value
                .map(|s| s.to_owned())
                .ok_or(std::env::VarError::NotPresent)
        }
    }

    fn fake_env(home: Option<&str>, userprofile: Option<&str>) -> FakeHomeEnvironment {
        FakeHomeEnvironment {
            home: home.map(String::from),
            userprofile: userprofile.map(String::from),
        }
    }

    #[test]
    fn resolve_home_dir_succeeds_with_home_set() {
        let env = fake_env(Some("/home/test"), None);
        let home = resolve_home_dir_with_environment(&env).unwrap();
        assert!(home.is_absolute());
    }

    #[test]
    fn resolve_home_dir_errors_when_home_unset() {
        let env = fake_env(None, None);
        let err = resolve_home_dir_with_environment(&env).unwrap_err();
        assert!(
            err.to_string().contains("neither HOME nor USERPROFILE"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_home_dir_errors_when_home_empty() {
        let env = fake_env(Some(""), None);
        let err = resolve_home_dir_with_environment(&env).unwrap_err();
        assert!(
            err.to_string().contains("must be an absolute path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_home_dir_errors_when_home_relative() {
        let env = fake_env(Some("relative/path"), None);
        let err = resolve_home_dir_with_environment(&env).unwrap_err();
        assert!(
            err.to_string().contains("must be an absolute path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_home_dir_falls_back_to_userprofile() {
        let env = fake_env(None, Some("/fallback"));
        let home = resolve_home_dir_with_environment(&env).unwrap();
        assert_eq!(home, PathBuf::from("/fallback"));
    }

    #[test]
    fn default_base_dir_appends_context_matters() {
        let env = fake_env(Some("/home/test"), None);
        let base = default_base_dir_with_environment(&env).unwrap();
        assert!(base.ends_with(".context-matters"));
        assert!(base.is_absolute());
    }
}
