//! Handler for the `cx_recall` tool.

use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit};
use cm_core::{ContextStore, EntryKind, ScopePath};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response, parse_params};

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

    // Capture query for hint generation (before move)
    let original_query = params.query.clone();

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

    // Build scope_hits as an object: { "global/project:helioy": 3, "global": 1 }
    let scope_hits: serde_json::Map<String, Value> = result
        .scope_hits
        .iter()
        .map(|(scope, count)| (scope.clone(), json!(count)))
        .collect();

    let mut response = json!({
        "results": results,
        "returned": results.len(),
        "scope_chain": result.scope_chain,
        "scope_hits": scope_hits,
    });

    if let Some(hint) = hint {
        response["hint"] = json!(hint);
    }

    json_response(response)
}
