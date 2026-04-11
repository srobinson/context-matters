//! Handler for the `cx_get` tool.

use cm_capabilities::projection::{format_get_view, project_web_get};
use cm_core::ContextStore;
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, dual_response, parse_params};

#[derive(Debug, Deserialize)]
struct CxGetParams {
    /// Entry IDs to retrieve. Maximum 100 per request. Each entry must be
    /// a full hyphenated UUIDv7 as rendered by `cx_recall` / `cx_browse`
    /// row headers.
    ids: Vec<String>,
}

pub async fn cx_get(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let params: CxGetParams = parse_params(args)?;

    if params.ids.is_empty() {
        return Err("Validation error: ids cannot be empty".to_owned());
    }
    if params.ids.len() > crate::mcp::MAX_BATCH_IDS {
        return Err(format!(
            "Validation error: maximum {} IDs per request",
            crate::mcp::MAX_BATCH_IDS
        ));
    }

    // Each input must be a full hyphenated UUIDv7. Anything that fails
    // `Uuid::parse_str` errors the whole batch so malformed input
    // surfaces crisply instead of silently missing rows.
    //
    // `canonical_ids` runs in lock-step with `uuids`: it carries the
    // string form that `format_get_view`'s missing-set diff compares
    // against `Entry::id.to_string()`. Normalizing to
    // `Uuid::to_string()` lets the caller type the UUID in any accepted
    // format (uppercase, no hyphens) and still match the formatter's
    // canonical render.
    let mut uuids: Vec<uuid::Uuid> = Vec::with_capacity(params.ids.len());
    let mut canonical_ids: Vec<String> = Vec::with_capacity(params.ids.len());
    for raw in &params.ids {
        let id = uuid::Uuid::parse_str(raw)
            .map_err(|e| format!("Validation error: invalid UUID '{raw}': {e}"))?;
        uuids.push(id);
        canonical_ids.push(id.to_string());
    }

    let entries = store.get_entries(&uuids).await.map_err(cm_err_to_string)?;

    // Both views diff `requested` against `Entry::id.to_string()` to
    // compute the missing list, so they receive `canonical_ids` (the
    // normalized `Uuid::to_string()` form) rather than whatever casing
    // or hyphenation the caller originally typed.
    let text = format_get_view(&entries, &canonical_ids);
    let view = project_web_get(&entries, &canonical_ids);
    dual_response(text, &view)
}
