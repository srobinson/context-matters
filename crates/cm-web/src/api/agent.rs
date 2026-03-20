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
use cm_store::CmStore;
use serde::{Deserialize, Serialize};
use url::form_urlencoded;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agent/recall", get(recall_handler))
        .route("/agent/browse", get(browse_handler))
}

// ── Shared query parsing ────────────────────────────────────────

#[derive(Debug)]
pub(crate) struct RecallQuery {
    pub query: Option<String>,
    pub scope: Option<String>,
    pub kinds: Vec<String>,
    pub tags: Vec<String>,
    pub limit: Option<u32>,
    pub max_tokens: Option<u32>,
}

pub(crate) fn parse_recall_query(raw: Option<&str>) -> Result<RecallQuery, ApiError> {
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

// ── Shared recall execution ─────────────────────────────────────

/// Intermediate result from executing a recall, shared by both
/// `/api/agent/recall` (with `_trace`) and `/api/entries/recall` (without).
pub(crate) struct RecallOutput {
    pub results: Vec<RecallEntryView>,
    pub returned: usize,
    pub scope_chain: Vec<String>,
    pub token_estimate: u32,
    pub routing: RecallRouting,
    pub candidates_before_filter: usize,
    pub fetch_limit_used: u32,
    pub post_filters_applied: Vec<String>,
    pub token_budget_exhausted: bool,
    pub hint: Option<String>,
}

/// Execute a recall against the store, returning all data needed by both
/// the agent endpoint and the compatibility alias.
pub(crate) async fn execute_recall(
    store: &CmStore,
    raw_query: Option<&str>,
) -> Result<RecallOutput, ApiError> {
    let rq = parse_recall_query(raw_query)?;

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

    // Capture query for hint generation (before move)
    let original_query = rq.query.clone();

    let mut post_filters_applied = Vec::new();
    if !kinds.is_empty() {
        post_filters_applied.push("kinds".to_owned());
    }
    if !rq.tags.is_empty() {
        post_filters_applied.push("tags".to_owned());
    }

    let result = recall::recall(
        store,
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

    // Build hint for zero-result queries with too many words
    let hint = if results.is_empty() {
        if let Some(ref q) = original_query {
            let word_count = q.split_whitespace().count();
            if word_count > 3 {
                Some(format!(
                    "Query has {word_count} words with implicit AND. Try fewer keywords (1-3) or use OR between synonyms. Example: instead of '{q}', try '{}'.",
                    q.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
                ))
            } else if word_count > 1 {
                Some("No matches. Try fewer keywords, prefix matching (e.g. 'migrat*'), or OR between synonyms.".to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    Ok(RecallOutput {
        returned: results.len(),
        results,
        scope_chain: result.scope_chain,
        token_estimate: result.token_estimate,
        routing: result.routing,
        candidates_before_filter: result.candidates_before_filter,
        fetch_limit_used: result.fetch_limit_used,
        post_filters_applied,
        token_budget_exhausted,
        hint,
    })
}

// ── Recall response types ───────────────────────────────────────

#[derive(Debug, Serialize)]
struct AgentRecallResponse {
    results: Vec<RecallEntryView>,
    returned: usize,
    scope_chain: Vec<String>,
    token_estimate: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
    _trace: RecallTrace,
}

#[derive(Debug, Serialize)]
struct RecallTrace {
    routing: String,
    candidates_before_filter: usize,
    fetch_limit_used: u32,
    post_filters_applied: Vec<String>,
    token_budget_exhausted: bool,
}

// ── Recall handler ──────────────────────────────────────────────

async fn recall_handler(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<AgentRecallResponse>, ApiError> {
    let output = execute_recall(&state.store, raw_query.0.as_deref()).await?;

    Ok(Json(AgentRecallResponse {
        returned: output.returned,
        results: output.results,
        scope_chain: output.scope_chain,
        token_estimate: output.token_estimate,
        hint: output.hint,
        _trace: RecallTrace {
            routing: match output.routing {
                RecallRouting::Search => "search".to_owned(),
                RecallRouting::TagScopeWalk => "tag_scope_walk".to_owned(),
                RecallRouting::ScopeResolve => "scope_resolve".to_owned(),
                RecallRouting::BrowseFallback => "browse_fallback".to_owned(),
            },
            candidates_before_filter: output.candidates_before_filter,
            fetch_limit_used: output.fetch_limit_used,
            post_filters_applied: output.post_filters_applied,
            token_budget_exhausted: output.token_budget_exhausted,
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
    next_cursor: Option<String>,
    _trace: BrowseTrace,
}

#[derive(Debug, Serialize)]
struct BrowseTrace {
    filter_set: BrowseFilterSet,
    sort: String,
}

#[derive(Debug, Serialize)]
struct BrowseFilterSet {
    scope_path: Option<String>,
    kind: Option<String>,
    tag: Option<String>,
    include_superseded: bool,
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

    let include_superseded = bq.include_superseded.unwrap_or(false);
    let limit = clamp_limit(bq.limit);

    // Echo back the actual filter values for the trace
    let filter_set = BrowseFilterSet {
        scope_path: scope_path.as_ref().map(|sp| sp.as_str().to_owned()),
        kind: kind.map(|k| k.as_str().to_owned()),
        tag: bq.tag.clone(),
        include_superseded,
    };

    let result = browse::browse(
        &state.store,
        BrowseRequest {
            scope_path,
            kind,
            tag: bq.tag,
            created_by: bq.created_by,
            include_superseded,
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
