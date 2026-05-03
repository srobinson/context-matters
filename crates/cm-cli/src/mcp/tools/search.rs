//! Handler for the `cx_search` tool.

use cm_capabilities::projection::{format_search_view, project_search_view};
use cm_capabilities::scope::{ScopeSelector, resolve_scope_filter};
use cm_capabilities::search;
use cm_capabilities::validation::{check_input_size, clamp_limit, parse_kind};
use cm_core::{ContentSearchRequest, ContextStore, EntryKind};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, dual_response, parse_params};
use crate::shared::reject_removed_scope_inputs;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CxSearchParams {
    query: String,
    scope: ScopeSelector,

    #[serde(default)]
    kinds: Vec<String>,

    #[serde(default)]
    tags: Vec<String>,

    #[serde(default)]
    limit: Option<u32>,

    #[serde(default)]
    cursor: Option<String>,
}

pub async fn cx_search(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    let params: CxSearchParams = parse_params(args)?;
    check_input_size(&params.query, "query")?;

    let kinds: Vec<EntryKind> = params
        .kinds
        .iter()
        .map(|k| parse_kind(k))
        .collect::<Result<Vec<_>, _>>()?;
    let kinds = (!kinds.is_empty()).then_some(kinds);
    let tags = (!params.tags.is_empty()).then_some(params.tags);
    let limit = clamp_limit(params.limit);
    let scope = resolve_scope_filter(store, &params.scope)
        .await
        .map_err(cm_err_to_string)?;

    let request = ContentSearchRequest {
        query: params.query,
        scope,
        kinds,
        tags,
        limit,
        cursor: params.cursor,
    };

    let page = search::search(store, request.clone())
        .await
        .map_err(cm_err_to_string)?;
    let view = project_search_view(&request.query, page);
    let text = format_search_view(&view);
    dual_response(text, &view)
}
