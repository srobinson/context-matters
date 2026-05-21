pub mod api;

use anyhow::Result;
use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use cm_store::CmStore;
use rust_embed::Embed;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 3141;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct Assets;

/// Shared application state passed to all handlers.
pub struct AppState {
    pub store: CmStore,
}

pub struct ServeOptions {
    pub open: bool,
    pub port: Option<u16>,
    pub host: Option<IpAddr>,
}

pub async fn serve(opts: ServeOptions) -> Result<()> {
    let port = opts.port.unwrap_or(DEFAULT_PORT);
    let host = opts.host.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

    tracing::info!(version = env!("CARGO_PKG_VERSION"), port, "cm-web starting",);

    let config = cm_store::load_config()?;
    tracing::info!(db = %config.db_path().display(), "opening store");
    let store = open_store_with_config(&config).await?;
    let state = Arc::new(AppState { store });

    let listener = TcpListener::bind(SocketAddr::new(host, port)).await?;
    tracing::info!("listening on http://localhost:{}", port);

    if opts.open {
        let url = format!("http://localhost:{port}");
        let _ = open::that(&url);
    }

    axum::serve(listener, app(state.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shutdown, running WAL checkpoint");
    if let Err(e) = cm_store::schema::wal_checkpoint(state.store.write_pool()).await {
        tracing::debug!(error = %e, "WAL checkpoint failed");
    }

    Ok(())
}

fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", api::router(state))
        .fallback(spa_handler)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &axum::http::Request<_>| {
                    let req_id = Uuid::now_v7();
                    let path = req.uri().path();
                    let is_api = path.starts_with("/api");
                    if is_api {
                        tracing::info_span!(
                            "http",
                            req_id = %req_id,
                            method = %req.method(),
                            path = %path,
                        )
                    } else {
                        tracing::debug_span!(
                            "http",
                            req_id = %req_id,
                            method = %req.method(),
                            path = %path,
                        )
                    }
                })
                .on_response(
                    |resp: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        let status = resp.status().as_u16();
                        let latency_ms = latency.as_millis() as u64;
                        if status >= 500 {
                            tracing::error!(status, latency_ms, "response");
                        } else if status >= 400 {
                            tracing::warn!(status, latency_ms, "response");
                        } else {
                            tracing::info!(status, latency_ms, "response");
                        }
                    },
                ),
        )
}

/// Serve embedded frontend assets with SPA fallback.
async fn spa_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if !path.is_empty()
        && let Some(file) = Assets::get(path)
    {
        let cache = if path.starts_with("assets/") {
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

async fn open_store_with_config(config: &cm_store::Config) -> Result<CmStore> {
    let db_path = config.db_path();

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

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("received shutdown signal");
}
