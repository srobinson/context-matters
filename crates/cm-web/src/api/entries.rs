//! Entry API handlers.
//!
//! `browse`, `search`, and `recall` share their parsing and capability
//! invocation with the `/api/agent/*` endpoints in [`super::agent`] so
//! the two HTTP prefixes always return identical [`WebBrowseView`] /
//! [`WebRecallView`] projections. `search` is a legacy alias that
//! routes through the recall capability (the query is treated as a
//! required FTS keyword and kind/tag narrow the scope walk).

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use cm_capabilities::projection::{WebBrowseView, WebRecallView, project_web_recall};
use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{
    ContextStore, Entry, EntryKind, EntryRelation, MutationSource, NewEntry, ScopePath,
    UpdateEntry, WriteContext,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::api::agent::{self, BrowseQuery, ExecutedRecall};
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

// ── Browse ──────────────────────────────────────────────────────

async fn browse(
    State(state): State<Arc<AppState>>,
    Query(bq): Query<BrowseQuery>,
) -> Result<Json<WebBrowseView>, ApiError> {
    let executed = agent::execute_browse(&state.store, bq).await?;
    Ok(Json(agent::project_executed_browse(&executed)))
}

// ── Search (legacy FTS alias, routed through recall) ────────────
//
// The historical contract of `/api/entries/search` was a required FTS
// keyword plus optional single-value kind/tag filters. Routing it
// through the `recall` capability preserves the wire shape while
// upgrading callers to the full scope-chain walk and tiered fallback
// that recall provides. The new response shape is `WebRecallView`,
// matching the semantic intent that "search is a subset of recall".

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
) -> Result<Json<WebRecallView>, ApiError> {
    if sq.q.is_empty() {
        return Err(ApiError(cm_core::CmError::Validation(
            "query parameter is required".to_owned(),
        )));
    }
    check_input_size(&sq.q, "query").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;

    let scope = sq
        .scope_path
        .map(|s| ScopePath::parse(&s))
        .transpose()
        .map_err(|e| ApiError(cm_core::CmError::InvalidScopePath(e)))?;

    let kinds: Vec<EntryKind> = sq
        .kind
        .as_deref()
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .transpose()?
        .map(|k| vec![k])
        .unwrap_or_default();

    let tags: Vec<String> = sq.tag.map(|t| vec![t]).unwrap_or_default();
    let limit = clamp_limit(sq.limit);

    let request = RecallRequest {
        query: Some(sq.q),
        scope,
        kinds,
        tags,
        limit,
        max_tokens: None,
    };

    let result = recall::recall(&state.store, request.clone())
        .await
        .map_err(ApiError)?;

    Ok(Json(project_web_recall(&result, &request)))
}

// ── Recall compatibility alias ──────────────────────────────────
//
// Delegates to the same shared `execute_recall` helper that
// `/api/agent/recall` uses. After the migration to `WebRecallView`
// the two endpoints are byte-for-byte identical; the compat alias
// is retained so pre-migration clients keep working at the legacy
// URL path.

async fn recall(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<WebRecallView>, ApiError> {
    let ExecutedRecall { result, request } =
        agent::execute_recall(&state.store, raw_query.0.as_deref()).await?;
    Ok(Json(project_web_recall(&result, &request)))
}

// ── Single entry operations ─────────────────────────────────────

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
