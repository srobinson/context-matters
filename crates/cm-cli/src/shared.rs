//! Helpers specific to the cm-cli MCP adapter layer.

use serde_json::Value;

/// Serialize a JSON value to a pretty-printed string for the response.
pub fn json_response(value: Value) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|e| format!("[json] {e}"))
}

/// Pass a pre-formatted YAML text response through unchanged.
///
/// Sibling to [`json_response`] for tools that already format YAML directly
/// via `cm_capabilities::projection::format_*_view` / `format_*_ack`.
pub fn yaml_response(text: String) -> Result<String, String> {
    Ok(text)
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
