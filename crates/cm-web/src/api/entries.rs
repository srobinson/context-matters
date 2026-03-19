//! Entry API handlers.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Json;
use axum::routing::get;
use cm_core::{
    BrowseSort, ContextStore, Entry, EntryFilter, EntryKind, PagedResult, Pagination, ScopePath,
};
use serde::Deserialize;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/entries", get(browse))
}

#[derive(Debug, Deserialize)]
struct BrowseQuery {
    scope_path: Option<String>,
    kind: Option<String>,
    tag: Option<String>,
    created_by: Option<String>,
    sort: Option<String>,
    include_superseded: Option<bool>,
    cursor: Option<String>,
    limit: Option<u32>,
}

async fn browse(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BrowseQuery>,
) -> Result<Json<PagedResult<Entry>>, ApiError> {
    let scope_path = q
        .scope_path
        .map(|s| ScopePath::parse(&s))
        .transpose()
        .map_err(|e| ApiError(cm_core::CmError::InvalidScopePath(e)))?;

    let kind = q.kind.map(|k| parse_entry_kind(&k)).transpose()?;

    let sort = q
        .sort
        .map(|s| parse_browse_sort(&s))
        .transpose()?
        .unwrap_or_default();

    let filter = EntryFilter {
        scope_path,
        kind,
        tag: q.tag,
        created_by: q.created_by,
        include_superseded: q.include_superseded.unwrap_or(false),
        sort,
        pagination: Pagination {
            limit: q.limit.unwrap_or(20).clamp(1, 200),
            cursor: q.cursor,
        },
    };

    let result = state.store.browse(filter).await?;
    Ok(Json(result))
}

fn parse_entry_kind(s: &str) -> Result<EntryKind, ApiError> {
    serde_json::from_value::<EntryKind>(serde_json::Value::String(s.to_owned()))
        .map_err(|_| ApiError(cm_core::CmError::InvalidEntryKind(s.to_owned())))
}

fn parse_browse_sort(s: &str) -> Result<BrowseSort, ApiError> {
    serde_json::from_value::<BrowseSort>(serde_json::Value::String(s.to_owned())).map_err(|_| {
        ApiError(cm_core::CmError::Validation(format!(
            "invalid sort: '{s}' (expected recent, oldest, title_asc, title_desc, scope_asc, scope_desc, kind_asc, kind_desc)"
        )))
    })
}
