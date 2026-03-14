//! MCP server implementation for context-matters.
//!
//! Manual JSON-RPC over stdio, following the same pattern as fmm.
//! No rmcp dependency. The protocol is simple enough that a library
//! adds more complexity than it removes.

mod schema;
pub mod tools;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use cm_core::{CmError, ScopePath};
use cm_store::CmStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// ── Constants ─────────────────────────────────────────────────────

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Maximum input size for text-accepting tool fields (1 MB).
pub(crate) const MAX_INPUT_BYTES: usize = 1_048_576;

/// Maximum number of IDs in a batch request.
#[allow(dead_code)]
pub(crate) const MAX_BATCH_IDS: usize = 100;

/// Default result limit for retrieval tools.
pub(crate) const DEFAULT_LIMIT: u32 = 20;

/// Maximum result limit.
pub(crate) const MAX_LIMIT: u32 = 200;

/// Snippet length for two-phase retrieval responses.
#[allow(dead_code)]
pub(crate) const SNIPPET_LENGTH: usize = 200;

// ── Server Instructions ───────────────────────────────────────────

const SERVER_INSTRUCTIONS: &str = "\
You have a structured context store for persistent project knowledge across sessions.

SESSION LIFECYCLE:
1. RECALL: At session start, call cx_recall with a summary of the user's task or question. \
   This returns relevant context entries (facts, decisions, preferences, lessons) from \
   the current scope and all ancestor scopes. Use returned context silently.
2. STORE: When you discover important facts, decisions, user preferences, lessons learned, \
   or recurring patterns, call cx_store to persist them. Classify entries by kind for \
   effective retrieval later.
3. FEEDBACK: When the user corrects you or clarifies a preference, store it as kind='feedback'. \
   Feedback entries receive highest recall priority.

TOOLS OVERVIEW:
- cx_recall: Search and retrieve context. Primary retrieval tool. Call at session start.
- cx_store: Store a single context entry with structured metadata.
- cx_deposit: Batch-store conversation exchanges for future reference.
- cx_browse: List entries with filters and pagination. For inventory, not search.
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
pub struct McpServer {
    store: Arc<CmStore>,
}

impl McpServer {
    /// Construct a new MCP server wrapping the given store.
    pub fn new(store: CmStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Access the underlying store (for WAL checkpoint on shutdown, etc.).
    pub fn store(&self) -> &CmStore {
        &self.store
    }

    /// Run the JSON-RPC stdio loop.
    ///
    /// Uses blocking `stdin.lock().lines()` inside an async fn.
    /// Safe for v1 because: (1) `run()` is called directly from main(),
    /// not via `tokio::spawn()`, so `Send` is not required; (2) the MCP
    /// stdio protocol is single-client sequential request/response;
    /// (3) there is no concurrent work to preempt.
    pub async fn run(&self) -> anyhow::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
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
                    writeln!(stdout, "{}", serde_json::to_string(&error_response)?)?;
                    stdout.flush()?;
                    continue;
                }
            };

            let response = self.handle_request(&request).await;

            if let Some(resp) = response {
                let write_result = writeln!(stdout, "{}", serde_json::to_string(&resp)?)
                    .and_then(|()| stdout.flush());
                if let Err(e) = write_result {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        return Ok(());
                    }
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = request.id.clone().unwrap_or(Value::Null);

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(),
            "notifications/initialized" => return None,
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
                "version": env!("CARGO_PKG_VERSION"),
                "instructions": SERVER_INSTRUCTIONS
            }
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
            "cx_recall" => tools::cx_recall(&self.store, &arguments),
            "cx_store" => tools::cx_store(&self.store, &arguments),
            "cx_deposit" => tools::cx_deposit(&self.store, &arguments),
            "cx_browse" => tools::cx_browse(&self.store, &arguments),
            "cx_get" => tools::cx_get(&self.store, &arguments),
            "cx_update" => tools::cx_update(&self.store, &arguments),
            "cx_forget" => tools::cx_forget(&self.store, &arguments),
            "cx_stats" => tools::cx_stats(&self.store, &arguments),
            "cx_export" => tools::cx_export(&self.store, &arguments),
            _ => Err(format!("Unknown tool: {tool_name}")),
        };

        match result {
            Ok(value) => Ok(json!({
                "content": [{"type": "text", "text": value}]
            })),
            Err(e) => Ok(json!({
                "content": [{"type": "text", "text": format!("ERROR: {e}")}],
                "isError": true
            })),
        }
    }
}

// ── Error Conversion ──────────────────────────────────────────────

/// Convert a `CmError` to an actionable error string with recovery guidance.
pub(crate) fn cm_err_to_string(e: CmError) -> String {
    match e {
        CmError::EntryNotFound(id) => {
            format!("Entry '{id}' not found. Verify the ID using cx_browse or cx_recall.")
        }
        CmError::ScopeNotFound(path) => {
            format!(
                "Scope '{path}' does not exist. Use cx_stats to list available scopes, \
                 or create it by storing an entry with a new scope_path."
            )
        }
        CmError::DuplicateContent(existing_id) => {
            format!(
                "Duplicate content: an active entry with this content already exists \
                 (id: {existing_id}). Use cx_update to modify the existing entry, \
                 or cx_forget it first."
            )
        }
        CmError::InvalidScopePath(e) => {
            format!(
                "Invalid scope_path: {e}. Format: 'global', 'global/project:<id>', \
                 'global/project:<id>/repo:<id>', or \
                 'global/project:<id>/repo:<id>/session:<id>'. \
                 Identifiers must be lowercase alphanumeric with hyphens."
            )
        }
        CmError::InvalidEntryKind(s) => {
            format!(
                "Invalid kind '{s}'. Valid values: fact, decision, preference, lesson, \
                 reference, feedback, pattern, observation."
            )
        }
        CmError::InvalidRelationKind(s) => {
            format!(
                "Invalid relation kind '{s}'. Valid values: supersedes, relates_to, \
                 contradicts, elaborates, depends_on."
            )
        }
        CmError::Validation(msg) => msg,
        CmError::ConstraintViolation(msg) => format!("Constraint violation: {msg}"),
        CmError::Json(e) => format!("[json] {e}"),
        CmError::Database(msg) => format!("[database] {msg}"),
        CmError::Internal(msg) => format!("[internal] {msg}"),
    }
}

// ── Helper Functions ──────────────────────────────────────────────

/// Reject input exceeding the per-field byte limit.
#[allow(dead_code)]
pub(crate) fn check_input_size(value: &str, field: &str) -> Result<(), String> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(format!("{field} exceeds {MAX_INPUT_BYTES} byte limit"));
    }
    Ok(())
}

/// Clamp a limit value to the allowed range `[1, MAX_LIMIT]`.
#[allow(dead_code)]
pub(crate) fn clamp_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Truncate body to a snippet, safe for multi-byte UTF-8.
///
/// Uses `floor_char_boundary` (stable since Rust 1.82) to avoid
/// panicking on multi-byte character boundaries. Tries to break
/// at a word boundary for readability.
#[allow(dead_code)]
pub(crate) fn snippet(body: &str, max_chars: usize) -> String {
    if body.len() <= max_chars {
        return body.to_owned();
    }
    let end = body.floor_char_boundary(max_chars);
    match body[..end].rfind(' ') {
        Some(pos) => format!("{}...", &body[..pos]),
        None => format!("{}...", &body[..end]),
    }
}

/// Rough token estimate: ~4 characters per token for English text.
#[allow(dead_code)]
pub(crate) fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(4)
}

/// Encode a `PaginationCursor` to a URL-safe base64 string.
#[allow(dead_code)]
pub(crate) fn encode_cursor(cursor: &cm_core::PaginationCursor) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let json = serde_json::to_string(cursor).expect("cursor serialization");
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// Decode a URL-safe base64 string to a `PaginationCursor`.
#[allow(dead_code)]
pub(crate) fn decode_cursor(encoded: &str) -> Result<cm_core::PaginationCursor, String> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "Invalid cursor format".to_owned())?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid cursor format".to_owned())
}

/// Serialize a JSON value to a pretty-printed string for the response.
#[allow(dead_code)]
pub(crate) fn json_response(value: Value) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|e| format!("[json] {e}"))
}

/// Ensure the full scope chain exists, creating missing scopes top-down.
///
/// When `cx_store` or `cx_deposit` receives a scope path that does not
/// exist, this function creates the full scope chain automatically. This
/// prevents agents from needing to manage scope creation separately.
#[allow(dead_code)]
pub(crate) fn ensure_scope_chain(store: &CmStore, path: &ScopePath) -> Result<(), String> {
    use cm_core::{ContextStore, NewScope};

    let ancestors: Vec<&str> = path.ancestors().collect();

    // Walk from root (last) to leaf (first)
    for ancestor_str in ancestors.into_iter().rev() {
        let ancestor = ScopePath::parse(ancestor_str).map_err(|e| cm_err_to_string(e.into()))?;
        match store.get_scope(&ancestor) {
            Ok(_) => continue,
            Err(CmError::ScopeNotFound(_)) => {
                // Derive label from the last segment
                let label = ancestor_str
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.split(':').nth(1))
                    .unwrap_or(ancestor_str)
                    .to_owned();

                let new_scope = NewScope {
                    path: ancestor,
                    label,
                    meta: None,
                };
                store.create_scope(new_scope).map_err(cm_err_to_string)?;
            }
            Err(e) => return Err(cm_err_to_string(e)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_limit_defaults_to_20() {
        assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
    }

    #[test]
    fn clamp_limit_caps_at_max() {
        assert_eq!(clamp_limit(Some(500)), MAX_LIMIT);
    }

    #[test]
    fn clamp_limit_floors_at_1() {
        assert_eq!(clamp_limit(Some(0)), 1);
    }

    #[test]
    fn clamp_limit_passes_through_valid() {
        assert_eq!(clamp_limit(Some(50)), 50);
    }

    #[test]
    fn snippet_short_text_unchanged() {
        assert_eq!(snippet("hello world", 200), "hello world");
    }

    #[test]
    fn snippet_truncates_at_word_boundary() {
        let long_text = "a ".repeat(150);
        let result = snippet(&long_text, 200);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 210); // 200 + "..."
    }

    #[test]
    fn estimate_tokens_rough_accuracy() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("abc"), 1); // 3 chars rounds up to 1 token
    }

    #[test]
    fn check_input_size_accepts_small() {
        assert!(check_input_size("hello", "field").is_ok());
    }

    #[test]
    fn check_input_size_rejects_large() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(check_input_size(&big, "body").is_err());
    }

    #[test]
    fn cursor_roundtrip() {
        let cursor = cm_core::PaginationCursor {
            updated_at: chrono::Utc::now(),
            id: uuid::Uuid::now_v7(),
        };
        let encoded = encode_cursor(&cursor);
        let decoded = decode_cursor(&encoded).unwrap();
        assert_eq!(decoded.id, cursor.id);
    }

    #[test]
    fn decode_cursor_rejects_garbage() {
        assert!(decode_cursor("not-valid-base64!@#").is_err());
    }

    #[test]
    fn cm_err_to_string_includes_recovery_guidance() {
        let err = CmError::EntryNotFound(uuid::Uuid::nil());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("cx_browse"));
        assert!(msg.contains("cx_recall"));

        let err = CmError::InvalidEntryKind("bogus".to_owned());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("fact"));
        assert!(msg.contains("decision"));
        assert!(msg.contains("observation"));
    }

    #[test]
    fn cm_err_to_string_scope_not_found_has_guidance() {
        let err = CmError::ScopeNotFound("global/project:foo".to_owned());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("cx_stats"));
    }

    #[test]
    fn cm_err_to_string_duplicate_has_guidance() {
        let err = CmError::DuplicateContent(uuid::Uuid::nil());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("cx_update"));
        assert!(msg.contains("cx_forget"));
    }
}
