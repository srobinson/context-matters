//! JSON export download endpoint.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use cm_core::{ContextStore, ScopePath};
use serde::Deserialize;

use crate::AppState;
use crate::api::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    scope_path: Option<String>,
}

pub async fn export(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let scope_path = q
        .scope_path
        .map(|s| ScopePath::parse(&s))
        .transpose()
        .map_err(|e| ApiError(cm_core::CmError::InvalidScopePath(e)))?;

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
