//! Export capability: JSON snapshot of the store, optionally filtered to an
//! exact scope.
//!
//! Lifts the format validation, scope-prefix filtering, and snapshot
//! assembly out of the `cx_export` MCP handler so both the MCP tool and
//! the `cm export` CLI handler share one implementation. The result is a
//! typed [`ExportView`] struct that derives [`serde::Serialize`]; both
//! channels serialize it via `serde_json::to_string_pretty` (CLI) or
//! `serde_json::to_value` (MCP), so the wire shape is byte-identical
//! across channels.
//!
//! Export is the only command where stdout is genuinely
//! machine-consumable: a snapshot taken via `cm export > backup.json`
//! must round-trip cleanly through `serde_json::from_str`. Keeping the
//! shape in one place is what makes that contract enforceable.

use chrono::{DateTime, Utc};
use cm_core::{CmError, ContextStore, Entry, Scope};
use serde::Serialize;

use crate::scope::{ScopeSelector, resolve_scope_selection};

/// Inputs to [`export`].
#[derive(Debug, Clone)]
pub struct ExportRequest {
    /// Filter to a specific scope (`None` = full export).
    pub scope: Option<ScopeSelector>,
    /// Output format. Currently only `"json"` is accepted; any other
    /// value short-circuits to [`CmError::Validation`] before any store
    /// access.
    pub format: String,
}

/// Snapshot of the store at a moment in time.
///
/// Field order matches the legacy MCP `cx_export` JSON shape so existing
/// backups remain forward-compatible:
///
/// 1. `entries` — every active entry in the requested exact scope
/// 2. `scopes` — every scope whose path string starts with the request
///    scope path (or all scopes when no filter was given)
/// 3. `exported_at` — server-side timestamp at the moment the snapshot
///    was assembled, RFC 3339 in JSON
/// 4. `count` — `entries.len()`, included for callers that want a header
///    figure without parsing the full array
#[derive(Debug, Serialize)]
pub struct ExportView {
    /// Active entries in the requested subtree.
    pub entries: Vec<Entry>,
    /// Scopes in the requested subtree (prefix match against the scope
    /// path string when [`ExportRequest::scope`] is `Some`).
    pub scopes: Vec<Scope>,
    /// Server-side snapshot timestamp.
    pub exported_at: DateTime<Utc>,
    /// `entries.len()`, surfaced as a top-level header.
    pub count: usize,
}

/// Snapshot the store as JSON.
///
/// Validates [`ExportRequest::format`] before any store access so callers
/// see early-failure input errors as [`CmError::Validation`] without a
/// half-built response. Currently the only accepted format is `"json"`;
/// other values are rejected with a message mirroring the MCP handler
/// this was lifted from.
///
/// Scope filtering happens in two layers:
///
/// * `store.export(scope_path)` filters entries server-side by exact scope.
/// * `list_scopes(None)` returns every scope, then a string `starts_with`
///   prefix match keeps only those inside the requested subtree.
///
/// The two-layer scheme matches the legacy MCP behaviour exactly. A
/// future optimisation could push the scope filter into the store, but
/// the current cost is negligible (scope rows are small) and parity is
/// the immediate goal.
pub async fn export(
    store: &impl ContextStore,
    request: ExportRequest,
) -> Result<ExportView, CmError> {
    if request.format != "json" {
        return Err(CmError::Validation(format!(
            "Unsupported export format '{}'. Currently only 'json' is supported.",
            request.format
        )));
    }

    let scope_path = match request.scope.as_ref() {
        Some(selector) => Some(resolve_scope_selection(store, selector).await?),
        None => None,
    };
    let scope_path = scope_path
        .as_ref()
        .map(|selection| selection.read_scope_path())
        .transpose()?;

    let entries = store.export(scope_path).await?;

    let all_scopes = store.list_scopes(None).await?;
    let scopes: Vec<Scope> = match scope_path {
        Some(sp) => all_scopes
            .into_iter()
            .filter(|s| s.path.as_str().starts_with(sp.as_str()))
            .collect(),
        None => all_scopes,
    };

    let count = entries.len();

    Ok(ExportView {
        entries,
        scopes,
        exported_at: Utc::now(),
        count,
    })
}
