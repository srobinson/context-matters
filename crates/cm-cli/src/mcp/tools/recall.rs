//! Handler for the `cx_recall` tool.

use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{ContextStore, EntryKind, ScopePath};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

use super::entry_to_recall_json;

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
    let params: CxRecallParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

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

    // Delegate to RecallCapability
    let result = recall::recall(
        store,
        RecallRequest {
            query: params.query,
            scope,
            kinds,
            tags: params.tags,
            limit,
            max_tokens: params.max_tokens,
        },
    )
    .await
    .map_err(cm_err_to_string)?;

    // Map entries through the legacy JSON projection for MCP envelope
    let results: Vec<Value> = result.entries.iter().map(entry_to_recall_json).collect();

    let response = json!({
        "results": results,
        "returned": results.len(),
        "scope_chain": result.scope_chain,
        "token_estimate": result.token_estimate,
    });

    json_response(response)
}
