//! Handler for the `cx_get` tool.

use cm_capabilities::projection::{format_get_view, project_web_get};
use cm_core::{CmError, ContextStore};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, dual_response, parse_params};

/// Maximum candidate set returned when a prefix resolves to more than one
/// entry. Bounds the "ambiguous prefix" error message so an overly broad
/// prefix (e.g., `"01"`) does not drag the entire entries table into a
/// user-facing string. Sized to comfortably fit the error response.
const AMBIGUOUS_PREFIX_LIMIT: u32 = 10;

#[derive(Debug, Deserialize)]
struct CxGetParams {
    /// Entry IDs to retrieve. Maximum 100 per request. Each entry may be
    /// a full hyphenated UUIDv7 or a prefix (≥ 8 hex chars) as surfaced
    /// by `cx_recall` / `cx_browse` row headers.
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

    // Resolve each input string to a concrete UUID. Inputs that parse as
    // a full UUID bypass the store; everything else is routed through
    // `resolve_id_prefix`. A prefix that matches zero rows is preserved
    // as a not-found marker so the `format_get_view` "missing:" list
    // surfaces the original input the caller typed. A prefix that
    // matches more than one row fails the whole batch with a listing so
    // the caller can retry with a longer prefix — partial success here
    // would silently hide disambiguation bugs.
    //
    // `canonical_ids` runs in lock-step with `uuids`: it carries the
    // string form that `format_get_view`'s missing-set diff compares
    // against `Entry::id.to_string()`. That diff is a plain string
    // comparison, so a prefix input must be rewritten to its resolved
    // canonical UUID before the formatter sees it, otherwise a
    // successfully resolved prefix would render as "missing" because
    // "019d7436" never equals "019d7436-e4a1-74a3-bf93-d688efb63ba4".
    let mut uuids: Vec<uuid::Uuid> = Vec::with_capacity(params.ids.len());
    let mut canonical_ids: Vec<String> = Vec::with_capacity(params.ids.len());
    for raw in &params.ids {
        match uuid::Uuid::parse_str(raw) {
            Ok(id) => {
                uuids.push(id);
                // Normalize to the canonical hyphenated lowercase form
                // so the missing-set diff matches regardless of how the
                // caller typed the UUID (e.g., uppercase, no hyphens).
                canonical_ids.push(id.to_string());
            }
            Err(_) => {
                let matches = store
                    .resolve_id_prefix(raw, AMBIGUOUS_PREFIX_LIMIT)
                    .await
                    .map_err(|e| match e {
                        CmError::Validation(msg) => format!("Validation error: {msg}"),
                        other => cm_err_to_string(other),
                    })?;
                match matches.len() {
                    0 => {
                        // Nil sentinel never matches a real entry. The
                        // `get_entries` call will return nothing for
                        // this slot, and `format_get_view` will report
                        // the original prefix string under `missing:`.
                        uuids.push(uuid::Uuid::nil());
                        canonical_ids.push(raw.clone());
                    }
                    1 => {
                        uuids.push(matches[0]);
                        canonical_ids.push(matches[0].to_string());
                    }
                    n => {
                        let listing = matches
                            .iter()
                            .map(|u| u.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let suffix = if n as u32 >= AMBIGUOUS_PREFIX_LIMIT {
                            format!(" (first {AMBIGUOUS_PREFIX_LIMIT} shown)")
                        } else {
                            String::new()
                        };
                        return Err(format!(
                            "Validation error: id prefix '{raw}' is ambiguous, {n} entries match{suffix}: [{listing}] — retry with a longer prefix or the full hyphenated UUID"
                        ));
                    }
                }
            }
        }
    }

    let entries = store.get_entries(&uuids).await.map_err(cm_err_to_string)?;

    // Both views diff `requested` against `Entry::id.to_string()` to
    // compute the missing list. They MUST receive `canonical_ids`, not
    // the raw `params.ids`, otherwise a successfully resolved 8-char
    // prefix renders as missing because the formatter compares
    // "019d7436" to "019d7436-e4a1-74a3-bf93-d688efb63ba4" and finds no
    // match. The same correctness rule applies to `project_web_get`,
    // which the JSON structuredContent channel consumes.
    let text = format_get_view(&entries, &canonical_ids);
    let view = project_web_get(&entries, &canonical_ids);
    dual_response(text, &view)
}
