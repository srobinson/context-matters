//! Entry API handlers.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use cm_capabilities::projection::project_recall_entry;
use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{
    BrowseSort, ContextStore, Entry, EntryFilter, EntryKind, EntryRelation, MutationSource,
    NewEntry, PagedResult, Pagination, ScopePath, UpdateEntry, WriteContext,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::form_urlencoded;
use uuid::Uuid;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/entries", get(browse).post(create_entry))
        .route("/entries/search", get(search))
        .route("/entries/recall", get(recall))
        .route("/entries/merge", axum::routing::post(merge_entry))
        .route(
            "/entries/{id}",
            get(get_entry).patch(update_entry).delete(forget_entry),
        )
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

#[derive(Debug, Deserialize)]
struct RecallQuery {
    query: Option<String>,
    scope: Option<String>,
    kinds: Vec<String>,
    tags: Vec<String>,
    limit: Option<u32>,
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct RecallResponse {
    results: Vec<Value>,
    returned: usize,
    scope_chain: Vec<String>,
    token_estimate: u32,
}

async fn recall(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<RecallResponse>, ApiError> {
    let rq = parse_recall_query(raw_query.0.as_deref())?;

    if let Some(ref q) = rq.query {
        check_input_size(q, "query").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }

    let scope = rq
        .scope
        .map(|s| ScopePath::parse(&s))
        .transpose()
        .map_err(|e| ApiError(cm_core::CmError::InvalidScopePath(e)))?;

    let kinds: Vec<EntryKind> = rq
        .kinds
        .iter()
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .collect::<Result<Vec<_>, _>>()?;

    let limit = clamp_limit(rq.limit);

    let result = recall::recall(
        &state.store,
        RecallRequest {
            query: rq.query,
            scope,
            kinds,
            tags: rq.tags,
            limit,
            max_tokens: rq.max_tokens,
        },
    )
    .await
    .map_err(ApiError)?;

    let results: Vec<Value> = result
        .entries
        .iter()
        .map(|e| serde_json::to_value(project_recall_entry(e)).expect("RecallEntryView serializes"))
        .collect();

    Ok(Json(RecallResponse {
        returned: results.len(),
        results,
        scope_chain: result.scope_chain,
        token_estimate: result.token_estimate,
    }))
}

fn parse_recall_query(raw_query: Option<&str>) -> Result<RecallQuery, ApiError> {
    let mut query = None;
    let mut scope = None;
    let mut kinds = Vec::new();
    let mut tags = Vec::new();
    let mut limit = None;
    let mut max_tokens = None;

    for (key, value) in form_urlencoded::parse(raw_query.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "query" => query = Some(value.into_owned()),
            "scope" => scope = Some(value.into_owned()),
            "kinds" => kinds.push(value.into_owned()),
            "tags" => tags.push(value.into_owned()),
            "limit" => {
                limit = Some(value.parse::<u32>().map_err(|_| {
                    ApiError(cm_core::CmError::Validation(format!(
                        "invalid limit: '{value}'"
                    )))
                })?)
            }
            "max_tokens" => {
                max_tokens = Some(value.parse::<u32>().map_err(|_| {
                    ApiError(cm_core::CmError::Validation(format!(
                        "invalid max_tokens: '{value}'"
                    )))
                })?)
            }
            _ => {}
        }
    }

    Ok(RecallQuery {
        query,
        scope,
        kinds,
        tags,
        limit,
        max_tokens,
    })
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
    tracing::info!(
        action = "update",
        entry_id = %entry.id,
        title = %entry.title,
        kind = %entry.kind,
        source = "web",
        "mutation",
    );
    Ok(Json(entry))
}

async fn forget_entry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let uuid = parse_uuid(&id)?;
    let ctx = WriteContext::new(MutationSource::Web);
    state.store.forget_entry(uuid, &ctx).await?;
    tracing::info!(
        action = "forget",
        entry_id = %uuid,
        source = "web",
        "mutation",
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct MergeRequest {
    old_id: String,
    new_entry: NewEntry,
}

async fn merge_entry(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MergeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let old_uuid = parse_uuid(&body.old_id)?;
    let ctx = WriteContext::new(MutationSource::Web);
    let entry = state
        .store
        .supersede_entry(old_uuid, body.new_entry, &ctx)
        .await?;
    tracing::info!(
        action = "supersede",
        entry_id = %entry.id,
        old_id = %old_uuid,
        title = %entry.title,
        source = "web",
        "mutation",
    );
    Ok((StatusCode::CREATED, Json(entry)))
}

async fn create_entry(
    State(state): State<Arc<AppState>>,
    Json(new_entry): Json<NewEntry>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = WriteContext::new(MutationSource::Web);
    let entry = state.store.create_entry(new_entry, &ctx).await?;
    tracing::info!(
        action = "create",
        entry_id = %entry.id,
        title = %entry.title,
        kind = %entry.kind,
        source = "web",
        "mutation",
    );
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
