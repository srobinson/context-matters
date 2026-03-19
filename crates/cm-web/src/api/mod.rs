//! API router and handler modules.

mod entries;
mod error;

use std::sync::Arc;

use axum::Router;

use crate::AppState;

/// Build the `/api` router with all endpoints.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new().merge(entries::router()).with_state(state)
}
