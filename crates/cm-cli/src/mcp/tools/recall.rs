//! Handler for the `cx_recall` tool.

use cm_capabilities::projection::format_recall_view;
use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{ContextStore, EntryKind, ScopePath};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{cm_err_to_string, parse_params, yaml_response};

/// Parameters for the `cx_recall` tool.
#[derive(Debug, Deserialize)]
struct CxRecallParams {
    #[serde(default)]
    query: Option<String>,

    #[serde(default)]
    scope: Option<String>,

    #[serde(default)]
    kinds: Vec<String>,

    #[serde(default)]
    tags: Vec<String>,

    #[serde(default)]
    limit: Option<u32>,

    #[serde(default)]
    max_tokens: Option<u32>,
}

pub async fn cx_recall(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let params: CxRecallParams = parse_params(args)?;

    // Validate query size if provided
    if let Some(ref q) = params.query {
        check_input_size(q, "query")?;
    }

    // Parse and validate scope path
    let scope = match &params.scope {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    // Parse kind filters
    let kinds: Vec<EntryKind> = params
        .kinds
        .iter()
        .map(|k| k.parse::<EntryKind>().map_err(cm_err_to_string))
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

    yaml_response(format_recall_view(&result, &request))
}
