//! Agent-parity API handlers.
//!
//! These endpoints mirror the MCP tool semantics (cx_recall, cx_browse) over HTTP,
//! producing structurally identical results so the web UI can offer an "agent view."

use std::sync::Arc;

use axum::Router;
use axum::extract::{RawQuery, State};
use axum::response::Json;
use axum::routing::get;
use cm_capabilities::projection::{RecallEntryView, project_recall_entry};
use cm_capabilities::recall::{self, RecallRequest, RecallRouting};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{EntryKind, ScopePath};
use serde::Serialize;
use url::form_urlencoded;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/agent/recall", get(recall_handler))
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
