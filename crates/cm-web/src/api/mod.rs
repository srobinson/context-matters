//! API router and handler modules.

mod entries;
mod error;
mod mutations;
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
        .route("/mutations", get(mutations::list_mutations))
        .with_state(state)
}
