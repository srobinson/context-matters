//! API router and handler modules.

mod entries;
mod error;
mod stats;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::AppState;

/// Build the `/api` router with all endpoints.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(entries::router())
        .route("/stats", get(stats::get_stats))
        .with_state(state)
}
