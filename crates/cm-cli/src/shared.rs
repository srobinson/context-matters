//! Helpers specific to the cm-cli MCP adapter layer.

use cm_capabilities::projection::{project_browse_entry, project_full_entry, project_recall_entry};
use cm_core::Entry;
use serde_json::Value;

/// Serialize a JSON value to a pretty-printed string for the response.
pub fn json_response(value: Value) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|e| format!("[json] {e}"))
}

// ── Parameter Parsing ────────────────────────────────────────────

/// Parse tool parameters from JSON with actionable error messages.
///
/// Wraps serde deserialization errors with hints for common mistakes
/// (e.g., passing a string instead of a JSON array for tags/kinds/ids).
pub fn parse_params<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("expected a sequence") {
            format!("Invalid parameters: {msg}. Hint: array fields (tags, kinds, ids) must be JSON arrays, e.g. [\"value1\", \"value2\"], not strings.")
        } else {
            format!("Invalid parameters: {msg}")
        }
    })
}

// ── Serde Defaults ────────────────────────────────────────────────

/// Serde default for scope_path fields.
pub fn default_scope() -> String {
    "global".to_owned()
}

/// Serde default for created_by fields.
pub fn default_created_by() -> String {
    "agent:claude-code".to_owned()
}

// ── Entry Formatting (delegates to cm-capabilities projections) ──

/// Convert an entry to the two-phase recall response format (snippet, not full body).
pub fn entry_to_recall_json(entry: &Entry) -> Value {
    serde_json::to_value(project_recall_entry(entry)).expect("RecallEntryView serializes")
}

/// Convert an entry to the browse response format (two-phase: snippet, not full body).
pub fn entry_to_browse_json(entry: &Entry) -> Value {
    serde_json::to_value(project_browse_entry(entry)).expect("BrowseEntryView serializes")
}

/// Convert an entry to the full response format (includes body).
pub fn entry_to_full_json(entry: &Entry) -> Value {
    serde_json::to_value(project_full_entry(entry)).expect("FullEntryView serializes")
}
