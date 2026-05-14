//! MCP server implementation for context-matters.
//!
//! Manual JSON-RPC over stdio, following the same pattern as fmm.
//! No rmcp dependency. The protocol is simple enough that a library
//! adds more complexity than it removes.

mod instructions;
mod panic_guard;
mod protocol;
mod response;
mod schema;
mod server;
pub mod tools;
mod transport;

pub use panic_guard::install_panic_hook;
pub use protocol::JsonRpcError;
pub use response::{MAX_MCP_RESPONSE_BYTES, apply_cap_for_tool, cap_response};
pub use server::McpServer;

// Re-export helpers for internal use by tool handlers.
pub(crate) use cm_capabilities::error::cm_err_to_string;

pub(crate) use crate::shared::{
    ToolResult, dual_response, json_response, parse_params, reject_removed_scope_inputs,
    reject_unknown_fields,
};
