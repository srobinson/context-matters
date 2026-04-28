//! Contract tests verifying semantic parity between cm-web HTTP endpoints
//! and the underlying capability layer.
//!
//! Each test seeds a fixture store, calls both the web endpoint (via the
//! axum test client) and the capability layer directly (the same path the
//! MCP tools use), projects the capability result through the same
//! `project_web_*` helpers the web handlers use, then asserts full JSON
//! equality. If the two shapes ever drift, these tests catch it.

#[path = "parity/browse.rs"]
mod browse;
#[path = "parity/headers.rs"]
mod headers;
#[path = "parity/pagination.rs"]
mod pagination;
#[path = "parity/recall.rs"]
mod recall;
#[path = "parity/scope_migration.rs"]
mod scope_migration;
#[path = "parity/support.rs"]
mod support;
