//! JSON export download endpoint.

use std::sync::Arc;

use axum::extract::{RawQuery, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use cm_capabilities::scope::resolve_scope_selection;
use cm_core::ContextStore;

use crate::AppState;
use crate::api::error::ApiError;
use crate::api::scope_query;

pub async fn export(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Response, ApiError> {
    let scope_selector = scope_query::parse_scope_query(raw_query.0.as_deref())?;
    let scope_path = match scope_selector.as_ref() {
        Some(selector) => {
            let selection = resolve_scope_selection(&state.store, selector)
                .await
                .map_err(ApiError)?;
            Some(selection.read_scope_path().map_err(ApiError)?.clone())
        }
        None => None,
    };

    let entries = state.store.export(scope_path.as_ref()).await?;
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| ApiError(cm_core::CmError::Internal(e.to_string())))?;

    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".to_owned(),
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"cm-export.json\"".to_owned(),
            ),
        ],
        json,
    )
        .into_response())
}
