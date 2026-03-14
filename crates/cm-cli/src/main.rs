use anyhow::Result;
use clap::{ColorChoice, Parser, Subcommand};

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

async fn cmd_serve() -> Result<()> {
    tracing::info!("context-matters v{}", env!("CARGO_PKG_VERSION"));

    // TODO: Load config, create data dir, run migrations, open pool
    // TODO: Construct McpServer and run stdio loop
    // For now, wait for shutdown signal
    tracing::info!("MCP server starting on stdio (stub, waiting for shutdown signal)");

    shutdown_signal().await;

    tracing::info!("shutdown signal received, exiting");
    // TODO: WAL checkpoint on shutdown
    Ok(())
}

async fn cmd_stats() -> Result<()> {
    // TODO: Open store and print stats
    println!("context-matters v{}", env!("CARGO_PKG_VERSION"));
    println!("Stats not yet implemented");
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("ctrl-c handler");
    }
}
