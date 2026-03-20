//! MCP server implementation for context-matters.
//!
//! Manual JSON-RPC over stdio, following the same pattern as fmm.
//! No rmcp dependency. The protocol is simple enough that a library
//! adds more complexity than it removes.

mod schema;
pub mod tools;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use cm_core::ContextStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// Re-export shared helpers for internal use by tool handlers.
pub(crate) use crate::shared::{
    MAX_BATCH_IDS, check_input_size, cm_err_to_string, ensure_scope_chain, json_response, snippet,
};

// ── Constants ─────────────────────────────────────────────────────

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

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
            Ok(value) => Ok(json!({
                "content": [{"type": "text", "text": value}]
            })),
            // WORKAROUND: Claude Code cancels all sibling parallel MCP tool calls when
            // any tool returns isError:true (Promise.all fail-fast, tracked at
            // anthropics/claude-code#22264). Drop the flag; prefix with ERROR: so the
            // LLM recognises failure from content alone. Revert when #22264 ships
            // Promise.allSettled for MCP tools.
            Err(e) => Ok(json!({
                "content": [{"type": "text", "text": format!("ERROR: {e}")}]
            })),
        }
    }
}
