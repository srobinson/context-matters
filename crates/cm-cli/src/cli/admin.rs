//! Admin command handlers: `init`, `serve`, and the shared store opener.
//!
//! These commands are not MCP tools, so they do not appear in `tools.toml`
//! or `generated_help.rs`. They keep their hand-written help text inline at
//! the clap variant declarations in [`super::cli_def`].

use anyhow::{Result, bail};
use cm_store::CmStore;

use crate::mcp;

/// Open the database, run migrations, and return a ready-to-use store.
pub async fn open_store() -> Result<CmStore> {
    let config = cm_store::load_config()?;
    let db_path = config.db_path();

    // Ensure the data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (write_pool, read_pool) = cm_store::schema::create_pools(&db_path).await?;
    cm_store::schema::run_migrations(&write_pool).await?;

    Ok(CmStore::new_with_scope_inference_strategy(
        write_pool,
        read_pool,
        config.scope_inference_strategy,
    ))
}

/// `cm init` — write a commented config file to either `~/.context-matters/`
/// (with `--global`) or the current working directory.
pub fn cmd_init(global: bool, force: bool) -> Result<()> {
    let path = if global {
        let base = cm_store::default_base_dir()?;
        std::fs::create_dir_all(&base)?;
        base.join(cm_store::CONFIG_FILENAME)
    } else {
        std::env::current_dir()?.join(cm_store::CONFIG_FILENAME)
    };

    if path.exists() && !force {
        bail!(
            "config file already exists: {}\nUse --force to overwrite.",
            path.display()
        );
    }

    std::fs::write(&path, cm_store::config_template())?;
    println!("{}", path.display());
    Ok(())
}

/// `cm serve` — start the MCP server on stdio. Installs the panic hook
/// before any handler runs and checkpoints the WAL on shutdown.
pub async fn cmd_serve() -> Result<()> {
    // Install the MCP panic hook before any handler runs. With this
    // in place, a panic in any tool handler is converted to a
    // JSON-RPC `-32603` error response by the run loop instead of
    // tearing down the server process. See crates/cm-cli/src/mcp/
    // panic_guard.rs for the capture mechanism.
    mcp::install_panic_hook();

    tracing::info!("context-matters v{}", crate::VERSION);

    let store = open_store().await?;
    let server = mcp::McpServer::new(store);

    tracing::info!("MCP server ready on stdio");
    server.run().await?;

    tracing::info!("shutdown, running WAL checkpoint");
    if let Err(e) = cm_store::schema::wal_checkpoint(server.store().write_pool()).await {
        tracing::debug!(error = %e, "WAL checkpoint failed");
    }

    Ok(())
}
