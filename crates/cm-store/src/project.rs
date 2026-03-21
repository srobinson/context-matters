//! Data directory setup for context-matters.
//!
//! Resolves the default base directory (`~/.context-matters`) and ensures
//! it exists on first run.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Resolve the user's home directory from environment variables.
///
/// Reads `HOME` (Unix) or `USERPROFILE` (Windows). Returns an error if
/// neither is set, or if the value is empty or a relative path.
pub fn resolve_home_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
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
    Ok(resolve_home_dir()?.join(".context-matters"))
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
