//! Library entry point for cm-cli, exposing the MCP server, CLI handlers,
//! shared helpers, and tool handlers for integration testing.

pub mod cli;
pub mod mcp;
pub(crate) mod shared;
pub mod tool_contracts;
pub mod tool_docs;

pub use shared::yaml_response;

/// Crate version exposed for diagnostics and version banners.
pub const VERSION: &str = env!("CONTEXT_MATTERS_VERSION");
