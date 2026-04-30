//! Agent-parity API handlers.
//!
//! These endpoints produce the `WebBrowseView` / `WebRecallView`
//! projection shapes that the MCP `cx_browse` / `cx_recall` tools
//! surface over their YAML channel, so the cm-web Curator UI and the
//! MCP adapter render the exact same mental model of the store.
//!
//! The shared `execute_*` helpers are also reused by the
//! `/api/entries/*` compatibility aliases in `entries.rs` so the two
//! HTTP prefixes cannot drift.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, RawQuery, State};
use axum::response::Json;
use axum::routing::get;
use cm_capabilities::browse::{self, BrowseRequest, BrowseResult};
use cm_capabilities::projection::{
    RecallRow, WebBrowseView, WebRecallView, estimate_tokens, project_web_browse,
    project_web_recall,
};
use cm_capabilities::recall::{self, RecallRequest, RecallResult};
use cm_capabilities::scope::{ScopeSelector, resolve_scope_filter};
use cm_capabilities::search;
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{BrowseSort, ContentSearchPage, ContentSearchRequest, ContextStore, EntryKind};
use cm_store::CmStore;
use serde::Deserialize;
use url::form_urlencoded;

use crate::AppState;
use crate::api::error::ApiError;
use crate::api::scope_query;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agent/recall", get(recall_handler))
        .route("/agent/search", get(search_handler))
        .route("/agent/browse", get(browse_handler))
}

// ── Shared recall query parsing ─────────────────────────────────

#[derive(Debug)]
pub(crate) struct RecallQuery {
    pub query: Option<String>,
    pub scope: Option<ScopeSelector>,
    pub kinds: Vec<String>,
    pub tags: Vec<String>,
    pub limit: Option<u32>,
    pub max_tokens: Option<u32>,
}

const RECALL_QUERY_KEYS: &[&str] = &["query", "scope", "kinds", "tags", "limit", "max_tokens"];

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
            "scope" => scope = Some(scope_query::parse_scope_value(value.into_owned())?),
            "cwd" => return Err(scope_query::err_cwd_removed()),
            "scope_path" => return Err(scope_query::err_scope_path_removed()),
            "scope_mode" => return Err(scope_query::err_scope_mode_removed()),
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
            other => return Err(scope_query::err_unknown_query_key(other, RECALL_QUERY_KEYS)),
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

/// Raw capability result paired with the exact `RecallRequest` that
/// produced it. `/api/agent/recall` and `/api/entries/recall` both
/// project this pair via [`project_web_recall`] so the two endpoints
/// cannot drift.
pub(crate) struct ExecutedRecall {
    pub result: RecallResult,
    pub request: RecallRequest,
}

/// Parse a raw recall query string, validate inputs, invoke the
/// `recall` capability, and return the result plus the originating
/// request. Shared by `/api/agent/recall` and `/api/entries/recall`.
pub(crate) async fn execute_recall(
    store: &CmStore,
    raw_query: Option<&str>,
) -> Result<ExecutedRecall, ApiError> {
    let rq = parse_recall_query(raw_query)?;

    if let Some(ref q) = rq.query {
        check_input_size(q, "query").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }

    let kinds: Vec<EntryKind> = rq
        .kinds
        .iter()
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .collect::<Result<Vec<_>, _>>()?;

    let limit = clamp_limit(rq.limit);

    let request = RecallRequest {
        query: rq.query,
        scope: rq.scope,
        kinds,
        tags: rq.tags,
        limit,
        max_tokens: rq.max_tokens,
    };

    let result = recall::recall(store, request.clone())
        .await
        .map_err(ApiError)?;

    Ok(ExecutedRecall { result, request })
}

// ── Recall handler ──────────────────────────────────────────────

async fn recall_handler(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<WebRecallView>, ApiError> {
    let ExecutedRecall { result, request } =
        execute_recall(&state.store, raw_query.0.as_deref()).await?;
    Ok(Json(project_web_recall(&result, &request)))
}

// ── Shared search execution ─────────────────────────────────────

/// Raw capability search result paired with a `RecallRequest` used only
/// for the transitional `WebRecallView` projection.
pub(crate) struct ExecutedSearch {
    pub result: RecallResult,
    pub request: RecallRequest,
}

pub(crate) async fn execute_search(
    store: &CmStore,
    raw_query: Option<&str>,
) -> Result<ExecutedSearch, ApiError> {
    let sq = scope_query::parse_search_query(raw_query)?;
    check_input_size(&sq.q, "query").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;

    let kind = sq
        .kind
        .as_deref()
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .transpose()?;
    let tags = sq.tag.map(|tag| vec![tag]);
    let scope_selector = sq.scope.unwrap_or(ScopeSelector::All);
    let include_scope_chain = matches!(
        &scope_selector,
        ScopeSelector::Path(_) | ScopeSelector::CwdInferred { .. }
    );
    let scope_filter = resolve_scope_filter(store, &scope_selector)
        .await
        .map_err(ApiError)?;
    let limit = clamp_limit(sq.limit);

    let request = ContentSearchRequest {
        query: sq.q,
        scope: scope_filter,
        kinds: kind.map(|kind| vec![kind]),
        tags,
        limit,
        cursor: None,
    };

    let page = search::search(store, request.clone())
        .await
        .map_err(ApiError)?;
    let result = search_page_to_recall_result(store, &request, page, include_scope_chain).await?;
    let request = RecallRequest {
        query: Some(request.query),
        scope: Some(scope_selector),
        kinds: request.kinds.unwrap_or_default(),
        tags: request.tags.unwrap_or_default(),
        limit,
        max_tokens: None,
    };

    Ok(ExecutedSearch { result, request })
}

async fn search_page_to_recall_result(
    store: &CmStore,
    request: &ContentSearchRequest,
    page: ContentSearchPage,
    include_scope_chain: bool,
) -> Result<RecallResult, ApiError> {
    let entries: Vec<RecallRow> = page
        .items
        .into_iter()
        .map(|item| RecallRow {
            entry: item.entry,
            score: Some(item.score),
        })
        .collect();
    let relation_count_ids = entries.iter().map(|row| row.entry.id).collect::<Vec<_>>();
    let relation_counts = store
        .count_relations_for(&relation_count_ids)
        .await
        .map_err(ApiError)?;
    let token_estimate = entries
        .iter()
        .map(|row| estimate_tokens(&row.entry.body))
        .sum();
    let (scope_chain, scope_hits) = search_scope_summary(&entries, include_scope_chain);

    Ok(RecallResult {
        candidates_before_filter: entries.len(),
        entries,
        scope_chain,
        scope_hits,
        token_estimate,
        routing: recall::RecallRouting::Search,
        tier: None,
        fetch_limit_used: request.limit,
        relation_counts,
        advisories: Vec::new(),
    })
}

fn search_scope_summary(
    rows: &[RecallRow],
    include_scope_chain: bool,
) -> (Vec<String>, Vec<(String, usize)>) {
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for row in rows {
        *seen
            .entry(row.entry.scope_path.as_str().to_owned())
            .or_default() += 1;
    }
    let mut hits: Vec<(String, usize)> = seen.into_iter().collect();
    hits.sort_by(|a, b| {
        let depth_a = a.0.matches('/').count();
        let depth_b = b.0.matches('/').count();
        depth_b.cmp(&depth_a).then_with(|| a.0.cmp(&b.0))
    });
    let chain = if include_scope_chain {
        hits.iter().map(|(scope, _)| scope.clone()).collect()
    } else {
        Vec::new()
    };
    (chain, hits)
}

async fn search_handler(
    State(state): State<Arc<AppState>>,
    raw_query: RawQuery,
) -> Result<Json<WebRecallView>, ApiError> {
    let ExecutedSearch { result, request } =
        execute_search(&state.store, raw_query.0.as_deref()).await?;
    Ok(Json(project_web_recall(&result, &request)))
}

// ── Shared browse parsing + execution ────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BrowseQuery {
    pub scope: Option<String>,
    pub scope_path: Option<String>,
    pub scope_mode: Option<String>,
    pub cwd: Option<String>,
    pub include_resolution: Option<bool>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub include_superseded: Option<bool>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// Raw capability browse result plus projection settings derived from
/// the request. `/api/agent/browse` and `/api/entries` both use this
/// wrapper so smart browse metadata exposure cannot drift.
pub(crate) struct ExecutedBrowse {
    pub result: BrowseResult,
}

pub(crate) fn project_executed_browse(executed: &ExecutedBrowse) -> WebBrowseView {
    project_web_browse(&executed.result)
}

/// Validate a parsed [`BrowseQuery`], convert it into a
/// [`BrowseRequest`], and invoke the `browse` capability. Shared by
/// `/api/agent/browse` and `/api/entries` so the two endpoints produce
/// the same projection. `sort` defaults to [`BrowseSort::Recent`]
/// when the caller omits it.
pub(crate) async fn execute_browse(
    store: &CmStore,
    bq: BrowseQuery,
) -> Result<ExecutedBrowse, ApiError> {
    if let Some(ref t) = bq.tag {
        check_input_size(t, "tag").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }
    if let Some(ref c) = bq.created_by {
        check_input_size(c, "created_by")
            .map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }

    if bq.scope_path.is_some() {
        return Err(scope_query::err_scope_path_removed());
    }
    if bq.scope_mode.is_some() {
        return Err(scope_query::err_scope_mode_removed());
    }
    if bq.cwd.is_some() {
        return Err(scope_query::err_cwd_removed());
    }
    let scope = scope_query::parse_optional_scope(bq.scope)?;

    let kind = bq
        .kind
        .as_deref()
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .transpose()?;

    let sort = bq
        .sort
        .as_deref()
        .map(parse_browse_sort)
        .transpose()?
        .unwrap_or(BrowseSort::Recent);

    let include_superseded = bq.include_superseded.unwrap_or(false);

    let result = browse::browse(
        store,
        BrowseRequest {
            scope,
            include_resolution: bq.include_resolution,
            kind,
            tag: bq.tag,
            created_by: bq.created_by,
            include_superseded,
            sort,
            limit: bq.limit,
            cursor: bq.cursor,
        },
    )
    .await
    .map_err(ApiError)?;

    Ok(ExecutedBrowse { result })
}

fn parse_browse_sort(s: &str) -> Result<BrowseSort, ApiError> {
    serde_json::from_value::<BrowseSort>(serde_json::Value::String(s.to_owned())).map_err(|_| {
        ApiError(cm_core::CmError::Validation(format!(
            "invalid sort: '{s}' (expected recent, oldest, title_asc, title_desc, scope_asc, scope_desc, kind_asc, kind_desc)"
        )))
    })
}

// ── Browse handler ──────────────────────────────────────────────

async fn browse_handler(
    State(state): State<Arc<AppState>>,
    Query(bq): Query<BrowseQuery>,
) -> Result<Json<WebBrowseView>, ApiError> {
    let executed = execute_browse(&state.store, bq).await?;
    Ok(Json(project_executed_browse(&executed)))
}
