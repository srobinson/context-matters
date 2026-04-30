//! Handler for the `cx_recall` tool.

use cm_capabilities::projection::{format_recall_view, project_web_recall};
use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit, parse_kind};
use cm_core::{ContextStore, EntryKind};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
};
use crate::shared::parse_structured_scope_selector;

/// Parameters for the `cx_recall` tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CxRecallParams {
    #[serde(default)]
    query: Option<String>,

    #[serde(default)]
    scope: Option<Value>,

    #[serde(default)]
    kinds: Vec<String>,

    #[serde(default)]
    tags: Vec<String>,

    #[serde(default)]
    limit: Option<u32>,

    #[serde(default)]
    max_tokens: Option<u32>,
}

pub async fn cx_recall(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    let params: CxRecallParams = parse_params(args)?;
    let scope = parse_structured_scope_selector(params.scope)?;

    // Validate query size if provided
    if let Some(ref q) = params.query {
        check_input_size(q, "query")?;
    }

    // Parse kind filters
    let kinds: Vec<EntryKind> = params
        .kinds
        .iter()
        .map(|k| parse_kind(k))
        .collect::<Result<Vec<_>, _>>()?;

    let limit = clamp_limit(params.limit);

    let request = RecallRequest {
        query: params.query,
        scope,
        kinds,
        tags: params.tags,
        limit,
        max_tokens: params.max_tokens,
    };

    let result = recall::recall(store, request.clone())
        .await
        .map_err(cm_err_to_string)?;

    let text = format_recall_view(&result, &request);
    let view = project_web_recall(&result, &request);
    dual_response(text, &view)
}
