//! Data directory setup for context-matters.
//!
//! Resolves the default base directory (`~/.context-matters`) and ensures
//! it exists on first run.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Returns the default base directory: `$HOME/.context-matters`.
///
/// Falls back to `./.context-matters` if neither `HOME` nor `USERPROFILE`
/// is set (unlikely outside CI containers).
pub fn default_base_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home).join(".context-matters")
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
