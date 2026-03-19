//! API router and shared handler types.

use std::sync::Arc;

use axum::{Router, response::Json, routing::get};
use serde_json::{Value, json};

use crate::AppState;

/// Build the `/api` router with all endpoints.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
