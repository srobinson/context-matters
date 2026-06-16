//! Read-only recall shadow canary endpoint.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::Json;
use cm_core::{RecallShadowListFilter, RecallShadowRow};
use serde::Deserialize;

use crate::AppState;
use crate::api::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct RecallShadowQuery {
    routing: Option<String>,
    scope_path: Option<String>,
    top1_changed: Option<bool>,
    limit: Option<u32>,
}

pub async fn list_recall_shadow(
    State(state): State<Arc<AppState>>,
    Query(q): Query<RecallShadowQuery>,
) -> Result<Json<Vec<RecallShadowRow>>, ApiError> {
    let filter = RecallShadowListFilter {
        routing: q.routing,
        scope_path: q.scope_path,
        top1_changed: q.top1_changed,
        limit: q.limit.unwrap_or(50).clamp(1, 200),
    };

    let records = state.store.list_recall_shadow(filter).await?;

    Ok(Json(records))
}
