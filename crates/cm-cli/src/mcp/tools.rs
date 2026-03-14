//! Tool handlers for the 9 `cx_*` tools.
//!
//! Each handler receives a reference to the store and the raw JSON arguments,
//! validates inputs, calls the appropriate `ContextStore` trait methods, and
//! returns a pretty-printed JSON string or an error message with recovery guidance.

use cm_core::{ContextStore, Entry, EntryKind, ScopePath};
use cm_store::CmStore;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    SNIPPET_LENGTH, check_input_size, clamp_limit, cm_err_to_string, estimate_tokens,
    json_response, snippet,
};

// ── cx_recall ────────────────────────────────────────────────────

/// Parameters for the `cx_recall` tool.
#[derive(Debug, Deserialize)]
struct CxRecallParams {
    /// FTS5 search query. When omitted, uses scope resolution instead.
    #[serde(default)]
    query: Option<String>,

    /// Scope path to search within. Defaults to "global".
    #[serde(default)]
    scope: Option<String>,

    /// Filter to specific entry kinds (OR semantics).
    #[serde(default)]
    kinds: Vec<String>,

    /// Filter to entries with any of these tags (OR semantics).
    #[serde(default)]
    tags: Vec<String>,

    /// Maximum number of entries to return.
    #[serde(default)]
    limit: Option<u32>,

    /// Maximum token budget for the response.
    #[serde(default)]
    max_tokens: Option<u32>,
}

pub fn cx_recall(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxRecallParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    // Validate query size if provided
    if let Some(ref q) = params.query {
        check_input_size(q, "query")?;
    }

    // Parse and validate scope path
    let scope_path = match &params.scope {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };
    let default_scope = ScopePath::global();
    let scope_ref = scope_path.as_ref().unwrap_or(&default_scope);

    // Parse kind filters
    let kind_filters: Vec<EntryKind> = params
        .kinds
        .iter()
        .map(|k| k.parse::<EntryKind>().map_err(cm_err_to_string))
        .collect::<Result<Vec<_>, _>>()?;

    let limit = clamp_limit(params.limit);

    // Fetch more than requested when post-filtering, to compensate for filtered-out entries
    let has_post_filter = !kind_filters.is_empty() || !params.tags.is_empty();
    let fetch_limit = if has_post_filter {
        limit.saturating_mul(3).min(super::MAX_LIMIT)
    } else {
        limit
    };

    // Route to search or resolve_context based on query presence
    let entries = match &params.query {
        Some(query) => store
            .search(query, Some(scope_ref), fetch_limit)
            .map_err(cm_err_to_string)?,
        None => store
            .resolve_context(scope_ref, &kind_filters, fetch_limit)
            .map_err(cm_err_to_string)?,
    };

    // Post-filter by kinds (only when using search path, since resolve_context handles kinds internally)
    let entries = if params.query.is_some() && !kind_filters.is_empty() {
        entries
            .into_iter()
            .filter(|e| kind_filters.contains(&e.kind))
            .collect()
    } else {
        entries
    };

    // Post-filter by tags
    let entries: Vec<Entry> = if params.tags.is_empty() {
        entries
    } else {
        entries
            .into_iter()
            .filter(|e| entry_has_any_tag(e, &params.tags))
            .collect()
    };

    // Apply limit after post-filtering
    let entries: Vec<Entry> = entries.into_iter().take(limit as usize).collect();

    // Build scope chain from the target scope
    let scope_chain: Vec<String> = scope_ref.ancestors().map(String::from).collect();

    // Build result entries with token budget tracking
    let mut results = Vec::with_capacity(entries.len());
    let mut total_tokens: u32 = 0;

    for entry in &entries {
        let entry_json = entry_to_recall_json(entry);
        let entry_tokens = estimate_tokens(&entry_json.to_string());

        if let Some(budget) = params.max_tokens
            && total_tokens + entry_tokens > budget && !results.is_empty() {
                break;
            }

        total_tokens += entry_tokens;
        results.push(entry_json);
    }

    let response = json!({
        "results": results,
        "returned": results.len(),
        "scope_chain": scope_chain,
        "token_estimate": total_tokens,
    });

    json_response(response)
}

/// Convert an entry to the two-phase recall response format (snippet, not full body).
fn entry_to_recall_json(entry: &Entry) -> Value {
    let mut result = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "snippet": snippet(&entry.body, SNIPPET_LENGTH),
        "created_by": &entry.created_by,
        "updated_at": entry.updated_at.to_rfc3339(),
    });

    if let Some(ref meta) = entry.meta {
        if !meta.tags.is_empty() {
            result["tags"] = json!(meta.tags);
        }
        if let Some(ref confidence) = meta.confidence {
            result["confidence"] = json!(confidence);
        }
    }

    result
}

/// Check whether an entry has any of the specified tags.
fn entry_has_any_tag(entry: &Entry, tags: &[String]) -> bool {
    match &entry.meta {
        Some(meta) => meta.tags.iter().any(|t| tags.contains(t)),
        None => false,
    }
}

// ── Stubs for remaining tools ────────────────────────────────────

pub fn cx_store(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_store not yet implemented".to_owned())
}

pub fn cx_deposit(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_deposit not yet implemented".to_owned())
}

pub fn cx_browse(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_browse not yet implemented".to_owned())
}

pub fn cx_get(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_get not yet implemented".to_owned())
}

pub fn cx_update(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_update not yet implemented".to_owned())
}

pub fn cx_forget(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_forget not yet implemented".to_owned())
}

pub fn cx_stats(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_stats not yet implemented".to_owned())
}

pub fn cx_export(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_export not yet implemented".to_owned())
}
