//! Subprocess MCP protocol tests.
//!
//! Spawn the `cm serve` binary, pipe JSON-RPC messages to stdin, assert on stdout.
//! Each test uses an isolated tempdir via `CM_DATA_DIR` to prevent cross-test interference.

mod common;

#[path = "mcp_protocol/basics.rs"]
mod basics;
#[path = "mcp_protocol/examples.rs"]
mod examples;
#[path = "mcp_protocol/export.rs"]
mod export;
#[path = "mcp_protocol/schema_conformance.rs"]
mod schema_conformance;
#[path = "mcp_protocol/scope_migration.rs"]
mod scope_migration;
#[path = "mcp_protocol/tool_calls.rs"]
mod tool_calls;
#[path = "mcp_protocol/tools_list.rs"]
mod tools_list;
