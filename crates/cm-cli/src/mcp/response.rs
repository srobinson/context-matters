//! MCP tool response shaping and context-byte caps.

use serde_json::{Value, json};

use crate::shared::ToolResult;

/// Byte cap applied to every MCP tool response except `cx_export`.
///
/// Large payloads get offloaded to disk by Claude Code, which defeats the
/// purpose of structured tool output. 16 KB is the deliberate ceiling for
/// `cx_*` rows: denser than fmm's 10 KB source-snippet cap because a single
/// recall / browse row carries more metadata per byte than a code excerpt.
pub const MAX_MCP_RESPONSE_BYTES: usize = 16 * 1024;

/// Trailing advisory appended to any capped response.
const TRUNCATE_ADVISORY: &str = "\n[Truncated: response exceeded 16 KB cap. \
Use cx_get(id=...) for full bodies or narrow your query.]";

/// Reserved bytes for JSON-RPC wrapper fields around tool error envelopes.
const TOOL_ERROR_RPC_OVERHEAD_RESERVE_BYTES: usize = 512;

/// Maximum bytes spent on the hidden duplicate error message in `_meta`.
const TOOL_ERROR_META_MESSAGE_BYTES: usize = 512;

/// Upstream Claude Code issue that forces MCP tool errors to stay in a
/// success envelope for now.
const TOOL_ERROR_WORKAROUND_UPSTREAM: &str = "anthropics/claude-code#22264";

/// TODO(ALP-1964): restore top-level `isError: true` once Claude Code
/// handles parallel MCP tools with Promise.allSettled semantics.
const TOOL_ERROR_WORKAROUND_CLEANUP: &str = "Restore top-level isError:true when Claude Code handles parallel MCP tools with Promise.allSettled.";

/// Clip `text` to `max_bytes`, preferring a newline boundary.
///
/// Algorithm:
/// * If `text.len() <= max_bytes`, return unchanged.
/// * Otherwise reserve room for [`TRUNCATE_ADVISORY`] inside `max_bytes`.
/// * Walk back from the body budget to the nearest UTF-8 char boundary.
/// * Cut just after the last `\n` at or before that boundary so the body ends
///   at a clean line break. If no newline exists in range, hard-cap at the
///   char boundary.
/// * Append [`TRUNCATE_ADVISORY`] while keeping the final string within
///   `max_bytes`.
pub fn cap_response(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let advisory = if TRUNCATE_ADVISORY.len() > max_bytes {
        &TRUNCATE_ADVISORY[..max_bytes]
    } else {
        TRUNCATE_ADVISORY
    };
    let body_budget = max_bytes.saturating_sub(advisory.len());

    // Walk back from the body budget to a valid UTF-8 boundary so slicing cannot panic.
    let mut safe_end = body_budget;
    while safe_end > 0 && !text.is_char_boundary(safe_end) {
        safe_end -= 1;
    }
    let cut = match text[..safe_end].rfind('\n') {
        Some(nl) => nl + 1,
        None => safe_end,
    };
    let mut result = text[..cut].to_owned();
    result.push_str(advisory);
    result
}

/// Apply the MCP response cap for a tool, unless the tool is opted out.
///
/// `cx_export` bypasses the cap because it is the fidelity backup path and
/// must emit complete JSON. Every other `cx_*` tool response is clipped to
/// [`MAX_MCP_RESPONSE_BYTES`] via [`cap_response`].
pub fn apply_cap_for_tool(tool_name: &str, text: String) -> String {
    if tool_name == "cx_export" {
        return text;
    }
    cap_response(text, MAX_MCP_RESPONSE_BYTES)
}

/// Build the MCP `CallToolResult` envelope for a successful tool call.
///
/// Maps a [`ToolResult`] to the MCP 2025-06-18 dual-channel wire shape:
/// - `content: [{type: "text", text: ...}]` for tools with a non-empty
///   text channel, or `content: []` for structured-only tools (`cx_export`)
/// - `structuredContent: {...}` when the tool supplies a JSON projection;
///   omitted only when a tool returns no structured projection
///
/// The text channel is clipped via [`apply_cap_for_tool`] to protect LLM
/// context bytes. The structured channel is uncapped; MCP clients consume it
/// separately and it does not land in the model prompt.
pub(super) fn build_envelope(tool_name: &str, tool_result: ToolResult) -> Value {
    let content = if tool_result.text.is_empty() {
        json!([])
    } else {
        let capped = apply_cap_for_tool(tool_name, tool_result.text);
        json!([{"type": "text", "text": capped}])
    };
    let mut envelope = serde_json::Map::new();
    envelope.insert("content".to_owned(), content);
    if let Some(structured) = tool_result.structured {
        envelope.insert("structuredContent".to_owned(), structured);
    }
    Value::Object(envelope)
}

/// Build the temporary success envelope used for tool-handler failures.
///
/// WORKAROUND: Claude Code cancels sibling parallel MCP tool calls when
/// any result carries top-level `isError: true` because its MCP client
/// currently uses Promise.all fail-fast behavior. Until
/// [`TOOL_ERROR_WORKAROUND_UPSTREAM`] is fixed, keep the response as a
/// successful JSON-RPC `tools/call` result and make the failure visible in
/// two ways:
/// - `content[0].text` starts with `ERROR:` for the LLM-facing channel.
/// - `_meta.cm_tool_error` exposes the failure programmatically without
///   triggering Claude Code's top-level `isError` handling.
pub(super) fn build_tool_error_envelope(message: String) -> Value {
    let (text_budget, meta_budget) = tool_error_field_budgets();
    let text = cap_response(format!("ERROR: {message}"), text_budget);
    let meta_message = cap_response(message, meta_budget);

    build_tool_error_envelope_parts(text, meta_message)
}

fn tool_error_field_budgets() -> (usize, usize) {
    let fixed_result_bytes = serde_json::to_string(&build_tool_error_envelope_parts(
        String::new(),
        String::new(),
    ))
    .map(|s| s.len())
    .unwrap_or(TOOL_ERROR_RPC_OVERHEAD_RESERVE_BYTES);
    let field_budget = MAX_MCP_RESPONSE_BYTES
        .saturating_sub(fixed_result_bytes + TOOL_ERROR_RPC_OVERHEAD_RESERVE_BYTES);
    let meta_budget = field_budget.min(TOOL_ERROR_META_MESSAGE_BYTES);
    let text_budget = field_budget.saturating_sub(meta_budget);

    (text_budget, meta_budget)
}

fn build_tool_error_envelope_parts(text: String, message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "_meta": {
            "cm_tool_error": {
                "is_error": true,
                "message": message,
                "suppressed_top_level_is_error": true,
                "upstream_issue": TOOL_ERROR_WORKAROUND_UPSTREAM,
                "cleanup": TOOL_ERROR_WORKAROUND_CLEANUP
            }
        }
    })
}
