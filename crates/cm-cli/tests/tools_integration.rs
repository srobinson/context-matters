//! Integration tests for mutating and end to end `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp file SQLite database, runs migrations,
//! and exercises tool handlers through the public `tools::cx_*` functions.
//! This validates the full stack from JSON params through the tool handler,
//! ContextStore, and SQLite.

mod common;

#[path = "tools_integration/deposit.rs"]
mod deposit;
#[path = "tools_integration/e2e.rs"]
mod e2e;
#[path = "tools_integration/export.rs"]
mod export;
#[path = "tools_integration/forget.rs"]
mod forget;
#[path = "tools_integration/store.rs"]
mod store;
#[path = "tools_integration/update.rs"]
mod update;
