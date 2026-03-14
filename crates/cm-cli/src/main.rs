mod mcp;

use anyhow::Result;
use clap::{ColorChoice, Parser, Subcommand};
use cm_store::CmStore;

#[derive(Parser)]
#[command(
    name = "cm",
    about = "Structured context store for AI agents",
    version,
    color = ColorChoice::Auto
)]
struct Cli {
    /// Enable verbose debug output
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start MCP server on stdio transport
    Serve,
    /// Show store statistics
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing (stderr only, never stdout: MCP uses stdout)
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .init();

    match &cli.command {
        Commands::Serve => cmd_serve().await,
        Commands::Stats => cmd_stats().await,
    }
}

/// Open the database, run migrations, and return a ready-to-use store.
async fn open_store() -> Result<CmStore> {
    let config = cm_store::load_config();
    let db_path = config.db_path();

    // Ensure the data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (write_pool, read_pool) = cm_store::schema::create_pools(&db_path).await?;
    cm_store::schema::run_migrations(&write_pool).await?;

    Ok(CmStore::new(write_pool, read_pool))
}

async fn cmd_serve() -> Result<()> {
    tracing::info!("context-matters v{}", env!("CARGO_PKG_VERSION"));

    let store = open_store().await?;
    let server = mcp::McpServer::new(store);

    tracing::info!("MCP server ready on stdio");
    server.run().await?;

    tracing::info!("shutdown, running WAL checkpoint");
    cm_store::schema::wal_checkpoint(server.store().write_pool())
        .await
        .ok();

    Ok(())
}

async fn cmd_stats() -> Result<()> {
    use cm_core::ContextStore;

    let store = open_store().await?;
    let stats = store.stats().map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("context-matters v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Active entries:     {}", stats.active_entries);
    println!("Superseded entries: {}", stats.superseded_entries);
    println!("Scopes:             {}", stats.scopes);
    println!("Relations:          {}", stats.relations);
    println!("Database size:      {} bytes", stats.db_size_bytes);

    if !stats.entries_by_kind.is_empty() {
        println!();
        println!("By kind:");
        let mut kinds: Vec<_> = stats.entries_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in kinds {
            println!("  {kind:15} {count}");
        }
    }

    if !stats.entries_by_scope.is_empty() {
        println!();
        println!("By scope:");
        let mut scopes: Vec<_> = stats.entries_by_scope.iter().collect();
        scopes.sort_by_key(|(path, _)| (*path).clone());
        for (scope, count) in scopes {
            println!("  {scope:40} {count}");
        }
    }

    cm_store::schema::wal_checkpoint(store.write_pool())
        .await
        .ok();
    Ok(())
}
