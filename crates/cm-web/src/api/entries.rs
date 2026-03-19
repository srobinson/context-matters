//! Entry API handlers.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use cm_core::{
    BrowseSort, ContextStore, Entry, EntryFilter, EntryKind, EntryRelation, MutationSource,
    NewEntry, PagedResult, Pagination, ScopePath, UpdateEntry, WriteContext,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/entries", get(browse).post(create_entry))
        .route("/entries/search", get(search))
        .route("/entries/{id}", get(get_entry).patch(update_entry))
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

#[derive(Debug, Deserialize)]
struct SearchQuery {
    #[serde(rename = "query")]
    q: String,
    scope_path: Option<String>,
    kind: Option<String>,
    tag: Option<String>,
    limit: Option<u32>,
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(sq): Query<SearchQuery>,
) -> Result<Json<PagedResult<Entry>>, ApiError> {
    if sq.q.is_empty() {
        return Err(ApiError(cm_core::CmError::Validation(
            "query parameter is required".to_owned(),
        )));
    }

    let scope_path = sq
        .scope_path
        .map(|s| ScopePath::parse(&s))
        .transpose()
        .map_err(|e| ApiError(cm_core::CmError::InvalidScopePath(e)))?;

    let kind_filter = sq.kind.map(|k| parse_entry_kind(&k)).transpose()?;
    let tag_filter = sq.tag;

    let has_post_filter = kind_filter.is_some() || tag_filter.is_some();
    let limit = sq.limit.unwrap_or(20).clamp(1, 200);
    let fetch_limit = if has_post_filter {
        limit.saturating_mul(3).min(200)
    } else {
        limit
    };

    let mut entries = state
        .store
        .search(&sq.q, scope_path.as_ref(), fetch_limit)
        .await?;

    if let Some(kind) = kind_filter {
        entries.retain(|e| e.kind == kind);
    }
    if let Some(ref tag) = tag_filter {
        entries.retain(|e| {
            e.meta
                .as_ref()
                .is_some_and(|m| m.tags.iter().any(|t| t == tag))
        });
    }

    entries.truncate(limit as usize);
    let total = entries.len() as u64;

    Ok(Json(PagedResult {
        items: entries,
        total,
        next_cursor: None,
    }))
}

#[derive(Debug, Serialize)]
struct EntryDetail {
    #[serde(flatten)]
    entry: Entry,
    relations_from: Vec<EntryRelation>,
    relations_to: Vec<EntryRelation>,
}

async fn get_entry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<EntryDetail>, ApiError> {
    let uuid = parse_uuid(&id)?;

    let entry = state.store.get_entry(uuid).await?;
    let (relations_from, relations_to) = tokio::try_join!(
        state.store.get_relations_from(uuid),
        state.store.get_relations_to(uuid),
    )?;

    Ok(Json(EntryDetail {
        entry,
        relations_from,
        relations_to,
    }))
}

async fn update_entry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<UpdateEntry>,
) -> Result<Json<Entry>, ApiError> {
    let uuid = parse_uuid(&id)?;
    let ctx = WriteContext::new(MutationSource::Web);
    let entry = state.store.update_entry(uuid, update, &ctx).await?;
    Ok(Json(entry))
}

async fn create_entry(
    State(state): State<Arc<AppState>>,
    Json(new_entry): Json<NewEntry>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = WriteContext::new(MutationSource::Web);
    let entry = state.store.create_entry(new_entry, &ctx).await?;
    Ok((StatusCode::CREATED, Json(entry)))
}

fn parse_uuid(s: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(s)
        .map_err(|_| ApiError(cm_core::CmError::Validation(format!("invalid UUID: '{s}'"))))
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
