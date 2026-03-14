//! Tool list schema for MCP `tools/list` response.
//!
//! Generated from `tools.toml` by `build.rs`. The `generated_schema.rs` file
//! is auto-generated and should not be edited manually.

use serde_json::Value;

#[path = "generated_schema.rs"]
mod generated_schema;

/// Return the MCP `tools/list` response payload.
pub(super) fn tool_list() -> Value {
    generated_schema::generated_tool_list()
}
