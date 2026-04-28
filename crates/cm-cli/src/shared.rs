//! Helpers specific to the cm-cli MCP adapter layer.

use serde::Serialize;
use serde_json::Value;

// ── Tool Result ───────────────────────────────────────────────────

/// Dual-channel MCP tool output.
///
/// The MCP `CallToolResult` envelope carries two parallel channels:
/// - `content[].text` — human-readable (YAML for `cx_*`), consumed by the LLM
/// - `structuredContent` — machine-readable JSON matching the tool's
///   declared `outputSchema`
///
/// Write tools emit text only. Read tools emit both. `cx_export` emits
/// structured only (`text` is empty; the envelope builder in
/// `mcp/mod.rs` emits `content: []` in that case).
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub text: String,
    pub structured: Option<Value>,
}

impl ToolResult {
    /// Text-only result. Used by write tools whose response is a pure
    /// YAML acknowledgement with no structured projection.
    pub fn text_only(text: String) -> Self {
        Self {
            text,
            structured: None,
        }
    }

    /// Dual-channel result. Used by read tools that emit YAML text plus
    /// a JSON projection matching the declared `outputSchema`.
    pub fn dual(text: String, structured: Value) -> Self {
        Self {
            text,
            structured: Some(structured),
        }
    }

    /// Structured-only result. Used by `cx_export` whose canonical form
    /// is JSON for backup/restore fidelity; the text channel stays empty
    /// and the envelope builder omits `content[0].text`.
    pub fn structured_only(structured: Value) -> Self {
        Self {
            text: String::new(),
            structured: Some(structured),
        }
    }
}

// ── Response Helpers ──────────────────────────────────────────────

/// Wrap a JSON value as a structured-only tool result.
///
/// Used by `cx_export` whose canonical form is JSON for backup/restore
/// fidelity. The text channel is left empty.
pub fn json_response(value: Value) -> Result<ToolResult, String> {
    Ok(ToolResult::structured_only(value))
}

/// Pass a pre-formatted YAML text response through as a text-only tool
/// result.
///
/// Sibling to [`json_response`] and [`dual_response`] for tools that
/// emit a pure YAML acknowledgement via
/// `cm_capabilities::projection::format_*_ack` (write tools only).
pub fn yaml_response(text: String) -> Result<ToolResult, String> {
    Ok(ToolResult::text_only(text))
}

/// Wrap a YAML text body plus a serialisable structured view as a
/// dual-channel tool result.
///
/// Used by read tools (`cx_recall`, `cx_browse`, `cx_get`, `cx_stats`)
/// which emit both the LLM-facing YAML snippet and a JSON projection
/// matching the tool's declared `outputSchema`. Serialises the view
/// with `serde_json::to_value`; returns a parse-error string on failure.
/// Failure is practically impossible because every projection type
/// derives `Serialize` on plain data, but the fallible return keeps the
/// handler signature uniform with the other response helpers and lets
/// serialisation bugs surface as `ERROR:` messages rather than panics.
pub fn dual_response<T: Serialize>(text: String, view: &T) -> Result<ToolResult, String> {
    let structured = serde_json::to_value(view).map_err(|e| format!("[json] {e}"))?;
    Ok(ToolResult::dual(text, structured))
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

/// Reject public scope selection inputs removed by ALP-2054.
pub fn reject_removed_scope_inputs(args: &Value) -> Result<(), String> {
    let Some(object) = args.as_object() else {
        return Ok(());
    };
    if object.contains_key("scope_path") {
        return Err("Invalid parameters: scope_path has been removed; use scope".to_owned());
    }
    if object.contains_key("scope_mode") {
        return Err("Invalid parameters: scope_mode has been removed".to_owned());
    }
    Ok(())
}

/// Reject unknown fields when a param struct cannot use deny_unknown_fields.
pub fn reject_unknown_fields(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let Some(object) = args.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("Invalid parameters: unknown field `{key}`"));
        }
    }
    Ok(())
}

// ── Serde Defaults ────────────────────────────────────────────────

/// Serde default for scope fields.
pub fn default_scope() -> String {
    "global".to_owned()
}

/// Serde default for created_by fields.
pub fn default_created_by() -> String {
    "agent:claude-code".to_owned()
}
