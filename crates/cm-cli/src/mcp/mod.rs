//! MCP server implementation for context-matters.
//!
//! Manual JSON-RPC over stdio, following the same pattern as fmm.
//! No rmcp dependency. The protocol is simple enough that a library
//! adds more complexity than it removes.

mod panic_guard;
mod schema;
pub mod tools;

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use cm_core::ContextStore;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub use panic_guard::install_panic_hook;

// Re-export helpers for internal use by tool handlers.
pub(crate) use cm_capabilities::error::cm_err_to_string;

pub(crate) use crate::shared::{
    ToolResult, dual_response, json_response, parse_params, yaml_response,
};

// ── Constants ─────────────────────────────────────────────────────

/// MCP protocol version.
///
/// Bumped to `2025-06-18` (ALP-1761) so the dual-channel envelope from
/// ALP-1760 and the per-tool `outputSchema` declarations from ALP-1759
/// land under a coherent advertised protocol. Clean break: there is no
/// fallback to `2024-11-05`. The dual-channel envelope is structurally
/// backward-compatible (new clients pick up `structuredContent`, old
/// clients ignore the unknown field), but the protocol version we
/// *advertise* is the version whose semantics we guarantee.
const PROTOCOL_VERSION: &str = "2025-06-18";

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

// ── Response Helpers ──────────────────────────────────────────────

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
    // Prefer cutting just after the last newline before safe_end; otherwise
    // hard-cap at safe_end.
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

// ── Server Instructions ───────────────────────────────────────────

const SERVER_INSTRUCTIONS: &str = "\
You have a structured context store for persistent project knowledge across sessions.

TASK WORKFLOW:
1. RECALL: After receiving a task, call cx_recall with a summary of what you are working on. \
   This returns relevant context entries (facts, decisions, preferences, lessons) from \
   the current scope and all ancestor scopes. Use returned context silently. \
   cx_recall is useful at any point during a session, not only after the initial task.
2. STORE: When you discover important facts, decisions, user preferences, lessons learned, \
   or recurring patterns, call cx_store to persist them. Classify entries by kind for \
   effective retrieval later.
3. FEEDBACK: When the user corrects you or clarifies a preference, store it as kind='feedback'. \
   Feedback entries receive highest recall priority.

TOOLS OVERVIEW:
- cx_recall: Search and retrieve context. Primary retrieval tool. Call after receiving a task.
- cx_store: Store a single context entry with structured metadata.
- cx_deposit: Batch-store conversation exchanges for future reference.
- cx_browse: List entries with filters and pagination. Defaults to inferred local scope.
- cx_get: Fetch full content for specific entry IDs (two-phase retrieval).
- cx_update: Partially update an existing entry.
- cx_forget: Soft-delete entries that are no longer relevant.
- cx_stats: View store statistics and scope breakdown.
- cx_export: Export entries as JSON for backup.

SCOPE MODEL:
Scopes form a hierarchy: global > project > repo > session. \
Context at broader scopes is visible at narrower scopes. \
When storing entries, use the narrowest appropriate scope. \
Global scope is for cross-project knowledge (user preferences, universal patterns). \
Project scope is for project-level decisions and conventions. \
Repo scope is for codebase-specific facts. \
Session scope is for ephemeral task context.

PRINCIPLES:
- Be selective. Store genuinely reusable knowledge, not routine observations.
- Classify accurately. The kind field drives recall priority and filtering.
- Use specific scope paths. Overly broad scoping pollutes recall for unrelated work.
- Do not mention the context system to the user unless asked.
- If cx_recall returns empty results, that is fine. The scope is new.";

// ── JSON-RPC Types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ── McpServer ─────────────────────────────────────────────────────

/// Manual JSON-RPC over stdio MCP server for context-matters.
pub struct McpServer<S: ContextStore> {
    store: Arc<S>,
}

impl<S: ContextStore> McpServer<S> {
    /// Construct a new MCP server wrapping the given store.
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Access the underlying store (for WAL checkpoint on shutdown, etc.).
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Run the JSON-RPC stdio loop.
    ///
    /// Reads newline-delimited JSON-RPC requests from Tokio stdin and
    /// writes responses through Tokio stdout. The MCP stdio protocol is
    /// single-client sequential request/response.
    ///
    /// Error isolation. Every handler invocation is wrapped in
    /// [`futures::FutureExt::catch_unwind`] so a panic in any tool
    /// handler is converted to a JSON-RPC `-32603` error response
    /// instead of unwinding through `main` and terminating the
    /// server. The panic hook installed by [`install_panic_hook`]
    /// captures the payload, location, and backtrace so the response
    /// can surface them to the client. Similarly, response
    /// serialization and stdout writes never propagate errors out of
    /// the loop: they fall back to a minimal synthetic error response
    /// on serialization failure, exit cleanly on `BrokenPipe`, and
    /// log-and-continue on any other I/O error. The invariant is
    /// simple: one bad request must never take the server down.
    pub async fn run(&self) -> anyhow::Result<()> {
        let stdin = BufReader::new(io::stdin());
        let mut lines = stdin.lines();
        let mut stdout = io::stdout();

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_owned(),
                        id: Value::Null,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {e}"),
                            data: None,
                        }),
                    };
                    if write_response(&mut stdout, &error_response)
                        .await
                        .is_broken_pipe()
                    {
                        return Ok(());
                    }
                    continue;
                }
            };

            let request_id = request.id.clone().unwrap_or(Value::Null);
            let handler_result = AssertUnwindSafe(self.handle_request(&request))
                .catch_unwind()
                .await;

            let response = match handler_result {
                Ok(resp) => resp,
                Err(_payload) => {
                    // The panic hook ran synchronously on the same
                    // thread before the unwind reached us, so the
                    // snapshot should be populated. In the pathological
                    // case where it is not (e.g. a panic raised by
                    // code that bypasses the hook), fall back to a
                    // minimal error envelope keyed off the request id
                    // so the client can still correlate the failure.
                    let snapshot = panic_guard::take_last_panic();
                    Some(panic_error_response(request_id, snapshot))
                }
            };

            if let Some(resp) = response
                && write_response(&mut stdout, &resp).await.is_broken_pipe()
            {
                return Ok(());
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = request.id.clone().unwrap_or(Value::Null);

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(),
            _ if request.method.starts_with("notifications/") => return None,
            "tools/list" => Ok(schema::tool_list()),
            "tools/call" => self.handle_tool_call(&request.params).await,
            "ping" => Ok(json!({})),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        };

        Some(match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id,
                result: Some(value),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id,
                result: None,
                error: Some(error),
            },
        })
    }

    fn handle_initialize(&self) -> Result<Value, JsonRpcError> {
        Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "cm",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": SERVER_INSTRUCTIONS
        }))
    }

    async fn handle_tool_call(&self, params: &Option<Value>) -> Result<Value, JsonRpcError> {
        let params = params.as_ref().ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing params".to_owned(),
            data: None,
        })?;

        let tool_name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing tool name".to_owned(),
                data: None,
            })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match tool_name {
            "cx_recall" => tools::cx_recall(&*self.store, &arguments).await,
            "cx_store" => tools::cx_store(&*self.store, &arguments).await,
            "cx_deposit" => tools::cx_deposit(&*self.store, &arguments).await,
            "cx_browse" => tools::cx_browse(&*self.store, &arguments).await,
            "cx_get" => tools::cx_get(&*self.store, &arguments).await,
            "cx_update" => tools::cx_update(&*self.store, &arguments).await,
            "cx_forget" => tools::cx_forget(&*self.store, &arguments).await,
            "cx_stats" => tools::cx_stats(&*self.store, &arguments).await,
            "cx_export" => tools::cx_export(&*self.store, &arguments).await,
            _ => Err(format!("Unknown tool: {tool_name}")),
        };

        match result {
            Ok(tool_result) => Ok(build_envelope(tool_name, tool_result)),
            Err(e) => Ok(build_tool_error_envelope(e)),
        }
    }
}

/// Outcome of a single response write. Used by [`write_response`] to
/// tell the run loop whether the transport is still usable.
enum WriteOutcome {
    /// Response written (or the write was a non-fatal, logged no-op).
    Ok,
    /// Peer closed the pipe. The run loop should exit cleanly.
    BrokenPipe,
}

impl WriteOutcome {
    fn is_broken_pipe(&self) -> bool {
        matches!(self, WriteOutcome::BrokenPipe)
    }
}

/// Write a JSON-RPC response to stdout with error-isolated framing.
///
/// Never propagates errors out of the run loop:
/// * Serialization failure (`serde_json::to_string` returning `Err`)
///   emits a fallback internal-error envelope keyed off the original
///   request id. If even the fallback fails to serialize, a static
///   last-resort JSON string is written instead.
/// * `BrokenPipe` on write returns [`WriteOutcome::BrokenPipe`] so the
///   loop can exit cleanly.
/// * Any other I/O error is logged via `tracing::error!` and swallowed;
///   the run loop continues on the next iteration.
async fn write_response<W>(stdout: &mut W, resp: &JsonRpcResponse) -> WriteOutcome
where
    W: AsyncWrite + Unpin,
{
    let serialized = match serde_json::to_string(resp) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "response serialization failed, emitting fallback");
            let fallback = JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: resp.id.clone(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: format!("Internal error: response serialization failed: {e}"),
                    data: None,
                }),
            };
            serde_json::to_string(&fallback).unwrap_or_else(|_| {
                r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"serialization failure"}}"#
                    .to_owned()
            })
        }
    };

    let write_result = async {
        stdout.write_all(serialized.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await
    }
    .await;

    match write_result {
        Ok(()) => WriteOutcome::Ok,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => WriteOutcome::BrokenPipe,
        Err(e) => {
            tracing::error!(error = %e, "stdout write failed, continuing");
            WriteOutcome::Ok
        }
    }
}

/// Build a JSON-RPC `-32603` error response for a caught handler panic.
///
/// Surfaces the captured panic snapshot (message, `file:line:column`,
/// backtrace) to the MCP client via the error `data` field so operators
/// can diagnose the failure without tailing the server logs. Falls back
/// to a generic envelope when the snapshot is missing (hook not
/// installed, or an unusual panic path that bypassed it).
fn panic_error_response(
    id: Value,
    snapshot: Option<panic_guard::PanicSnapshot>,
) -> JsonRpcResponse {
    let (message, data) = match snapshot {
        Some(snap) => {
            let location = snap.location.as_deref().unwrap_or("<unknown>");
            let message = format!(
                "Internal error: handler panicked at {location}: {}",
                snap.message
            );
            let data = json!({
                "panic_message": snap.message,
                "panic_location": snap.location,
                "backtrace": snap.backtrace,
            });
            (message, data)
        }
        None => (
            "Internal error: handler panicked (no capture available)".to_owned(),
            json!({}),
        ),
    };

    JsonRpcResponse {
        jsonrpc: "2.0".to_owned(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code: -32603,
            message,
            data: Some(data),
        }),
    }
}

/// Build the MCP `CallToolResult` envelope for a successful tool call.
///
/// Maps a [`ToolResult`] to the MCP 2025-06-18 dual-channel wire shape:
/// - `content: [{type: "text", text: ...}]` for tools with a non-empty
///   text channel, or `content: []` for structured-only tools (`cx_export`)
/// - `structuredContent: {...}` when the tool supplies a JSON projection;
///   omitted entirely for text-only write tools
///
/// The text channel is clipped via [`apply_cap_for_tool`] to protect LLM
/// context bytes. The structured channel is uncapped — MCP clients
/// consume it separately and it does not land in the model prompt.
fn build_envelope(tool_name: &str, tool_result: ToolResult) -> Value {
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
fn build_tool_error_envelope(message: String) -> Value {
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
