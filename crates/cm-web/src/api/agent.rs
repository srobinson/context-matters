//! Agent-parity API handlers.
//!
//! These endpoints mirror the MCP tool semantics (cx_recall, cx_browse) over HTTP,
//! producing structurally identical results so the web UI can offer an "agent view."

use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, RawQuery, State};
use axum::response::Json;
use axum::routing::get;
use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{
    BrowseEntryView, RecallEntryView, project_browse_entry, project_recall_entry,
};
use cm_capabilities::recall::{self, RecallRequest, RecallRouting};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{BrowseSort, EntryKind, ScopePath};
use serde::{Deserialize, Serialize};
use url::form_urlencoded;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agent/recall", get(recall_handler))
        .route("/agent/browse", get(browse_handler))
}

// ── Query parsing ────────────────────────────────────────────────

#[derive(Debug)]
struct RecallQuery {
    query: Option<String>,
    scope: Option<String>,
    kinds: Vec<String>,
    tags: Vec<String>,
    limit: Option<u32>,
    max_tokens: Option<u32>,
}

fn parse_recall_query(raw: Option<&str>) -> Result<RecallQuery, ApiError> {
    let mut query = None;
    let mut scope = None;
    let mut kinds = Vec::new();
    let mut tags = Vec::new();
    let mut limit = None;
    let mut max_tokens = None;

    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
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

// ── Response types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct AgentRecallResponse {
    results: Vec<RecallEntryView>,
    returned: usize,
    scope_chain: Vec<String>,
    token_estimate: u32,
    _trace: RecallTrace,
}

#[derive(Debug, Serialize)]
struct RecallTrace {
    routing: String,
    candidates_before_filter: usize,
    fetch_limit_used: u32,
    token_budget_exhausted: bool,
}

// ── Handler ──────────────────────────────────────────────────────

async fn recall_handler(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<AgentRecallResponse>, ApiError> {
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

    let entries_len = result.entries.len();
    let results: Vec<RecallEntryView> = result.entries.iter().map(project_recall_entry).collect();

    let token_budget_exhausted = rq.max_tokens.is_some_and(|budget| {
        result.token_estimate >= budget && entries_len < result.candidates_before_filter
    });

    Ok(Json(AgentRecallResponse {
        returned: results.len(),
        results,
        scope_chain: result.scope_chain,
        token_estimate: result.token_estimate,
        _trace: RecallTrace {
            routing: match result.routing {
                RecallRouting::Search => "search".to_owned(),
                RecallRouting::TagScopeWalk => "tag_scope_walk".to_owned(),
                RecallRouting::ScopeResolve => "scope_resolve".to_owned(),
                RecallRouting::BrowseFallback => "browse_fallback".to_owned(),
            },
            candidates_before_filter: result.candidates_before_filter,
            fetch_limit_used: result.fetch_limit_used,
            token_budget_exhausted,
        },
    }))
}

// ── Browse ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct BrowseQuery {
    scope_path: Option<String>,
    kind: Option<String>,
    tag: Option<String>,
    created_by: Option<String>,
    include_superseded: Option<bool>,
    limit: Option<u32>,
    cursor: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentBrowseResponse {
    entries: Vec<BrowseEntryView>,
    total: u64,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    _trace: BrowseTrace,
}

#[derive(Debug, Serialize)]
struct BrowseTrace {
    filter_set: Vec<String>,
    sort: String,
}

async fn browse_handler(
    State(state): State<Arc<AppState>>,
    Query(bq): Query<BrowseQuery>,
) -> Result<Json<AgentBrowseResponse>, ApiError> {
    if let Some(ref t) = bq.tag {
        check_input_size(t, "tag").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }
    if let Some(ref c) = bq.created_by {
        check_input_size(c, "created_by")
            .map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }

    let scope_path = bq
        .scope_path
        .map(|s| ScopePath::parse(&s))
        .transpose()
        .map_err(|e| ApiError(cm_core::CmError::InvalidScopePath(e)))?;

    let kind = bq
        .kind
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .transpose()?;

    let limit = clamp_limit(bq.limit);

    // Track which filters are active for the trace
    let mut filter_set = Vec::new();
    if scope_path.is_some() {
        filter_set.push("scope_path".to_owned());
    }
    if kind.is_some() {
        filter_set.push("kind".to_owned());
    }
    if bq.tag.is_some() {
        filter_set.push("tag".to_owned());
    }
    if bq.created_by.is_some() {
        filter_set.push("created_by".to_owned());
    }
    if bq.include_superseded.unwrap_or(false) {
        filter_set.push("include_superseded".to_owned());
    }

    let result = browse::browse(
        &state.store,
        BrowseRequest {
            scope_path,
            kind,
            tag: bq.tag,
            created_by: bq.created_by,
            include_superseded: bq.include_superseded.unwrap_or(false),
            sort: BrowseSort::Recent,
            limit,
            cursor: bq.cursor,
        },
    )
    .await
    .map_err(ApiError)?;

    let entries: Vec<BrowseEntryView> = result.entries.iter().map(project_browse_entry).collect();

    Ok(Json(AgentBrowseResponse {
        total: result.total,
        has_more: result.has_more,
        next_cursor: result.next_cursor,
        entries,
        _trace: BrowseTrace {
            filter_set,
            sort: "recent".to_owned(),
        },
    }))
}
