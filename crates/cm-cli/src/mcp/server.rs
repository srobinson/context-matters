//! MCP JSON-RPC stdio server and method dispatch.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use cm_core::ContextStore;
use futures::FutureExt;
use serde_json::{Value, json};
use tokio::io::{self, AsyncBufReadExt, BufReader};

use super::instructions::SERVER_INSTRUCTIONS;
use super::panic_guard;
use super::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, PROTOCOL_VERSION};
use super::response::{build_envelope, build_tool_error_envelope};
use super::transport::write_response;
use super::{schema, tools};

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
    /// server. The panic hook installed by [`panic_guard::install_panic_hook`]
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
                    // The panic hook runs synchronously before the unwind reaches us.
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
                "version": crate::VERSION
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
            "cx_search" => tools::cx_search(&*self.store, &arguments).await,
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

/// Build a JSON-RPC `-32603` error response for a caught handler panic.
///
/// Surfaces the captured panic snapshot (message, `file:line:column`,
/// backtrace) to the MCP client via the error `data` field so operators
/// can diagnose the failure without tailing the server logs. Falls back
/// to a generic envelope when the snapshot is missing.
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
