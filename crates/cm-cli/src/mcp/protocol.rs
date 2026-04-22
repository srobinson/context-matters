//! JSON-RPC wire types for the MCP server.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP protocol version.
///
/// Bumped to `2025-06-18` (ALP-1761) so the dual-channel envelope from
/// ALP-1760 and the per-tool `outputSchema` declarations from ALP-1759
/// land under a coherent advertised protocol. Clean break: there is no
/// fallback to `2024-11-05`. The dual-channel envelope is structurally
/// backward-compatible (new clients pick up `structuredContent`, old
/// clients ignore the unknown field), but the protocol version we
/// advertise is the version whose semantics we guarantee.
pub(super) const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    pub(crate) _jsonrpc: String,
    pub(crate) id: Option<Value>,
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcResponse {
    pub(crate) jsonrpc: String,
    pub(crate) id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
