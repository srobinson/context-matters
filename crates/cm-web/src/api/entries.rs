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
use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{
    ContextStore, Entry, EntryKind, EntryMeta, EntryRelation, MutationSource, NewEntry,
    UpdateEntry, WriteContext,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::form_urlencoded;
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
    q: String,
    scope: Option<String>,
    cwd: Option<String>,
    kind: Option<String>,
    tag: Option<String>,
    limit: Option<u32>,
}

async fn search(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<WebRecallView>, ApiError> {
    let sq = parse_search_query(raw_query.0.as_deref())?;
    if sq.q.is_empty() {
        return Err(ApiError(cm_core::CmError::Validation(
            "query parameter is required".to_owned(),
        )));
    }
    check_input_size(&sq.q, "query").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;

    let scope = agent::parse_scope_selector(sq.scope, sq.cwd)?;

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

const SEARCH_QUERY_KEYS: &[&str] = &["query", "scope", "cwd", "kind", "tag", "limit"];

fn parse_search_query(raw: Option<&str>) -> Result<SearchQuery, ApiError> {
    let mut q = None;
    let mut scope = None;
    let mut cwd = None;
    let mut kind = None;
    let mut tag = None;
    let mut limit = None;

    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "query" => q = Some(value.into_owned()),
            "scope" => scope = Some(value.into_owned()),
            "cwd" => cwd = Some(value.into_owned()),
            "scope_path" => return Err(agent::err_scope_path_removed()),
            "scope_mode" => return Err(agent::err_scope_mode_removed()),
            "kind" => kind = Some(value.into_owned()),
            "tag" => tag = Some(value.into_owned()),
            "limit" => {
                limit = Some(value.parse::<u32>().map_err(|_| {
                    ApiError(cm_core::CmError::Validation(format!(
                        "invalid limit: '{value}'"
                    )))
                })?)
            }
            other => return Err(agent::err_unknown_query_key(other, SEARCH_QUERY_KEYS)),
        }
    }

    Ok(SearchQuery {
        q: q.unwrap_or_default(),
        scope,
        cwd,
        kind,
        tag,
        limit,
    })
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
