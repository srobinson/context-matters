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

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Query, RawQuery, State};
use axum::response::Json;
use axum::routing::get;
use cm_capabilities::browse::{self, BrowseRequest, BrowseResult};
use cm_capabilities::projection::{
    WebBrowseView, WebRecallView, project_web_browse, project_web_recall,
};
use cm_capabilities::recall::{self, RecallRequest, RecallResult};
use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{BrowseSort, EntryKind};
use cm_store::CmStore;
use serde::Deserialize;
use url::form_urlencoded;

use crate::AppState;
use crate::api::error::ApiError;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agent/recall", get(recall_handler))
        .route("/agent/browse", get(browse_handler))
}

// ── Shared recall query parsing ─────────────────────────────────

#[derive(Debug)]
pub(crate) struct RecallQuery {
    pub query: Option<String>,
    pub scope: Option<String>,
    pub cwd: Option<String>,
    pub kinds: Vec<String>,
    pub tags: Vec<String>,
    pub limit: Option<u32>,
    pub max_tokens: Option<u32>,
}

pub(crate) fn err_scope_path_removed() -> ApiError {
    ApiError(cm_core::CmError::Validation(
        "use 'scope' instead of 'scope_path'".to_owned(),
    ))
}

pub(crate) fn err_scope_mode_removed() -> ApiError {
    ApiError(cm_core::CmError::Validation(
        "use 'scope' instead of 'scope_mode'".to_owned(),
    ))
}

pub(crate) fn err_unknown_query_key(key: &str, allowed: &[&str]) -> ApiError {
    ApiError(cm_core::CmError::Validation(format!(
        "unknown query parameter '{key}' (allowed: {})",
        allowed.join(", ")
    )))
}

const SCOPE_QUERY_KEYS: &[&str] = &["scope", "cwd"];

pub(crate) fn parse_scope_query(
    raw: Option<&str>,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let mut scope = None;
    let mut cwd = None;

    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "scope" => scope = Some(value.into_owned()),
            "cwd" => cwd = Some(value.into_owned()),
            "scope_path" => return Err(err_scope_path_removed()),
            "scope_mode" => return Err(err_scope_mode_removed()),
            other => return Err(err_unknown_query_key(other, SCOPE_QUERY_KEYS)),
        }
    }

    Ok((scope, cwd))
}

pub(crate) fn parse_scope_selector(
    scope: Option<String>,
    cwd: Option<String>,
) -> Result<Option<ScopeSelector>, ApiError> {
    if let Some(ref s) = scope {
        check_input_size(s, "scope").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }
    if let Some(ref c) = cwd {
        check_input_size(c, "cwd").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }

    let cwd = parse_cwd(cwd)?;
    let Some(scope) = scope else {
        if cwd.is_some() {
            return Err(ApiError(cm_core::CmError::Validation(
                "cwd can only be supplied with scope='cwd_inferred'".to_owned(),
            )));
        }
        return Ok(None);
    };

    ScopeSelector::parse(&scope)
        .and_then(|selector| selector.with_cwd(cwd))
        .map(Some)
        .map_err(ApiError)
}

fn parse_cwd(cwd: Option<String>) -> Result<Option<PathBuf>, ApiError> {
    match cwd {
        Some(raw) if raw.trim().is_empty() => Err(ApiError(cm_core::CmError::Validation(
            "cwd cannot be empty".to_owned(),
        ))),
        Some(raw) => Ok(Some(raw.into())),
        None => Ok(None),
    }
}

const RECALL_QUERY_KEYS: &[&str] = &[
    "query",
    "scope",
    "cwd",
    "kinds",
    "tags",
    "limit",
    "max_tokens",
];

pub(crate) fn parse_recall_query(raw: Option<&str>) -> Result<RecallQuery, ApiError> {
    let mut query = None;
    let mut scope = None;
    let mut cwd = None;
    let mut kinds = Vec::new();
    let mut tags = Vec::new();
    let mut limit = None;
    let mut max_tokens = None;

    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "query" => query = Some(value.into_owned()),
            "scope" => scope = Some(value.into_owned()),
            "cwd" => cwd = Some(value.into_owned()),
            "scope_path" => return Err(err_scope_path_removed()),
            "scope_mode" => return Err(err_scope_mode_removed()),
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
            other => return Err(err_unknown_query_key(other, RECALL_QUERY_KEYS)),
        }
    }

    Ok(RecallQuery {
        query,
        scope,
        cwd,
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

    let scope = parse_scope_selector(rq.scope, rq.cwd)?;

    let kinds: Vec<EntryKind> = rq
        .kinds
        .iter()
        .map(|k| k.parse::<EntryKind>().map_err(ApiError))
        .collect::<Result<Vec<_>, _>>()?;

    let limit = clamp_limit(rq.limit);

    let request = RecallRequest {
        query: rq.query,
        scope,
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
    if let Some(ref s) = bq.scope {
        check_input_size(s, "scope").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }
    if let Some(ref c) = bq.cwd {
        check_input_size(c, "cwd").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    }

    if bq.scope_path.is_some() {
        return Err(err_scope_path_removed());
    }
    if bq.scope_mode.is_some() {
        return Err(err_scope_mode_removed());
    }
    let cwd = parse_cwd(bq.cwd)?;
    let scope = ScopeSelector::from_optional_scope(bq.scope.as_deref(), cwd).map_err(ApiError)?;

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
