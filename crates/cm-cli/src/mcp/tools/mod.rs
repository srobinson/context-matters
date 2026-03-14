//! Tool handlers for the 9 `cx_*` tools.
//!
//! Each handler receives a reference to the store and the raw JSON arguments,
//! validates inputs, calls the appropriate `ContextStore` trait methods, and
//! returns a pretty-printed JSON string or an error message with recovery guidance.

mod browse;
mod deposit;
mod export;
mod forget;
mod get;
mod recall;
mod stats;
mod store;
mod update;

pub use browse::cx_browse;
pub use deposit::cx_deposit;
pub use export::cx_export;
pub use forget::cx_forget;
pub use get::cx_get;
pub use recall::cx_recall;
pub use stats::cx_stats;
pub use store::cx_store;
pub use update::cx_update;

use cm_core::{Confidence, Entry};
use serde_json::{Value, json};

use super::{SNIPPET_LENGTH, snippet};

// ── Shared helpers ───────────────────────────────────────────────

/// Parse a confidence string to the Confidence enum.
pub(crate) fn parse_confidence(s: &str) -> Result<Confidence, String> {
    match s {
        "high" => Ok(Confidence::High),
        "medium" => Ok(Confidence::Medium),
        "low" => Ok(Confidence::Low),
        other => Err(format!(
            "Invalid confidence '{other}'. Valid values: high, medium, low."
        )),
    }
}

/// Serde default for scope_path fields.
pub(crate) fn default_scope() -> String {
    "global".to_owned()
}

/// Serde default for created_by fields.
pub(crate) fn default_created_by() -> String {
    "agent:claude-code".to_owned()
}

/// Convert an entry to the two-phase recall response format (snippet, not full body).
pub(crate) fn entry_to_recall_json(entry: &Entry) -> Value {
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

/// Convert an entry to the browse response format (two-phase: snippet, not full body).
pub(crate) fn entry_to_browse_json(entry: &Entry) -> Value {
    let mut result = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "snippet": snippet(&entry.body, SNIPPET_LENGTH),
        "created_by": &entry.created_by,
        "created_at": entry.created_at.to_rfc3339(),
        "updated_at": entry.updated_at.to_rfc3339(),
        "superseded_by": entry.superseded_by.map(|id| id.to_string()),
    });

    if let Some(ref meta) = entry.meta
        && !meta.tags.is_empty()
    {
        result["tags"] = json!(meta.tags);
    }

    result
}

/// Convert an entry to the full response format (includes body).
pub(crate) fn entry_to_full_json(entry: &Entry) -> Value {
    json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "body": &entry.body,
        "content_hash": &entry.content_hash,
        "meta": &entry.meta,
        "created_by": &entry.created_by,
        "created_at": entry.created_at.to_rfc3339(),
        "updated_at": entry.updated_at.to_rfc3339(),
        "superseded_by": entry.superseded_by.map(|id| id.to_string()),
    })
}

/// Check whether an entry has any of the specified tags.
pub(crate) fn entry_has_any_tag(entry: &Entry, tags: &[String]) -> bool {
    match &entry.meta {
        Some(meta) => meta.tags.iter().any(|t| tags.contains(t)),
        None => false,
    }
}
