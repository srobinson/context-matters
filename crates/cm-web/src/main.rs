use anyhow::Result;
use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use clap::Parser;
use cm_store::CmStore;
use rust_embed::Embed;
use std::sync::Arc;
use tokio::net::TcpListener;

mod api;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct Assets;

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
        .nest("/api", api::router(state.clone()))
        .fallback(spa_handler)
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

/// Serve embedded frontend assets with SPA fallback.
async fn spa_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try exact file match first
    if !path.is_empty()
        && let Some(file) = Assets::get(path) {
            let cache = if path.starts_with("assets/") {
                // Vite hashed assets: cache forever
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type(path)),
                    (header::CACHE_CONTROL, cache.to_owned()),
                ],
                file.data.into_owned(),
            )
                .into_response();
        }

    // SPA fallback: serve index.html for client-side routing
    match Assets::get("index.html") {
        Some(file) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8".to_owned()),
                (header::CACHE_CONTROL, "no-cache".to_owned()),
            ],
            file.data.into_owned(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "frontend not built").into_response(),
    }
}

fn content_type(path: &str) -> String {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
    .to_owned()
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
