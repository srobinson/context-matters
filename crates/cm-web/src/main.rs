use anyhow::Result;
use axum::{Router, response::Json};
use clap::Parser;
use cm_store::CmStore;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::net::TcpListener;

mod api;

#[derive(Parser)]
#[command(
    name = "cm-web",
    about = "Context-matters web monitoring interface",
    version
)]
struct Cli {
    /// Port to listen on
    #[arg(long, default_value = "3141")]
    port: u16,

    /// Open browser after starting
    #[arg(long)]
    open: bool,

    /// Enable verbose debug output
    #[arg(long)]
    verbose: bool,
}

/// Shared application state passed to all handlers.
pub struct AppState {
    pub store: CmStore,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("cm-web v{}", env!("CARGO_PKG_VERSION"));

    let store = open_store().await?;
    let state = Arc::new(AppState { store });

    let app = Router::new()
        .route("/", axum::routing::get(root_handler))
        .nest("/api", api::router(state.clone()))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{}", cli.port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://localhost:{}", cli.port);

    if cli.open {
        let url = format!("http://localhost:{}", cli.port);
        let _ = open::that(&url);
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shutdown, running WAL checkpoint");
    cm_store::schema::wal_checkpoint(state.store.write_pool())
        .await
        .ok();

    Ok(())
}

async fn root_handler() -> Json<Value> {
    Json(json!({
        "name": "cm-web",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "ok"
    }))
}

async fn open_store() -> Result<CmStore> {
    let config = cm_store::load_config();
    let db_path = config.db_path();

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (write_pool, read_pool) = cm_store::schema::create_pools(&db_path).await?;
    cm_store::schema::run_migrations(&write_pool).await?;

    Ok(CmStore::new(write_pool, read_pool))
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("received shutdown signal");
}
