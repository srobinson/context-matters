//! Entry API handlers.
//!
//! `browse`, `search`, and `recall` share their parsing and capability
//! invocation with the `/api/agent/*` endpoints in [`super::agent`] so
//! the two HTTP prefixes always return identical [`WebBrowseView`] /
//! [`WebRecallView`] projections.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use cm_capabilities::projection::{WebBrowseView, WebRecallView, project_web_recall};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{
    ContextStore, Entry, EntryKind, EntryMeta, EntryRelation, MutationSource, NewEntry,
    UpdateEntry, WriteContext,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::AppState;
use crate::api::agent::{self, BrowseQuery, ExecutedRecall, ExecutedSearch};
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

// ── Search ──────────────────────────────────────────────────────

async fn search(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<WebRecallView>, ApiError> {
    let ExecutedSearch { result, request } =
        agent::execute_search(&state.store, raw_query.0.as_deref()).await?;
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
    new_entry: EntryWriteRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryWriteRequest {
    scope: String,
    kind: EntryKind,
    title: String,
    body: String,
    created_by: String,
    #[serde(default)]
    meta: Option<EntryMeta>,
}

impl TryFrom<EntryWriteRequest> for NewEntry {
    type Error = ApiError;

    fn try_from(value: EntryWriteRequest) -> Result<Self, Self::Error> {
        let scope_path = match ScopeSelector::parse(&value.scope).map_err(ApiError)? {
            ScopeSelector::Path(scope_path) => scope_path,
            ScopeSelector::CwdInferred { .. } => {
                return Err(ApiError(cm_core::CmError::Validation(
                    "scope='cwd_inferred' is not supported for entry write bodies".to_owned(),
                )));
            }
            ScopeSelector::Subtree(_) | ScopeSelector::Set(_) | ScopeSelector::All => {
                return Err(ApiError(cm_core::CmError::Validation(
                    "entry write bodies require scope kind 'path'".to_owned(),
                )));
            }
        };

        Ok(NewEntry {
            scope_path,
            kind: value.kind,
            title: value.title,
            body: value.body,
            created_by: value.created_by,
            meta: value.meta,
        })
    }
}

async fn merge_entry(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, ApiError> {
    let body: MergeRequest = parse_json_body(body)?;
    let old_uuid = parse_uuid(&body.old_id)?;
    let ctx = WriteContext::new(MutationSource::Web);
    let new_entry = body.new_entry.try_into()?;
    let entry = state
        .store
        .supersede_entry(old_uuid, new_entry, &ctx)
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
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, ApiError> {
    let new_entry: NewEntry = parse_json_body::<EntryWriteRequest>(body)?.try_into()?;
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

fn parse_json_body<T>(body: Value) -> Result<T, ApiError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(body).map_err(|e| {
        ApiError(cm_core::CmError::Validation(format!(
            "invalid request body: {e}"
        )))
    })
}
