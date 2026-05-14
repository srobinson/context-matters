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
use cm_core::{CmError, ContextStore, Entry, Scope, ScopeFilter, ScopePath};
use serde::Serialize;

use crate::scope::{ScopeSelector, resolve_scope_filter, resolve_scope_selection};

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
    /// Active entries in the requested scope filter.
    pub entries: Vec<Entry>,
    /// Scopes in the requested scope filter.
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
/// Exact selectors are pushed into the store. Broad selectors load active
/// entries once and filter them in memory, matching the existing all-scope
/// export path while preserving `descendants`, `set`, and `all` semantics.
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

    let scope_filter = match request.scope.as_ref() {
        Some(selector @ (ScopeSelector::Path(_) | ScopeSelector::CwdInferred { .. })) => {
            let selection = resolve_scope_selection(store, selector).await?;
            ScopeFilter::Exact(selection.read_scope_path()?.clone())
        }
        Some(
            selector @ (ScopeSelector::Subtree(_) | ScopeSelector::Set(_) | ScopeSelector::All),
        ) => resolve_scope_filter(store, selector).await?,
        None => ScopeFilter::All,
    };

    let entries = match &scope_filter {
        ScopeFilter::Exact(scope_path) => store.export(Some(scope_path)).await?,
        ScopeFilter::All => store.export(None).await?,
        filter => store
            .export(None)
            .await?
            .into_iter()
            .filter(|entry| scope_filter_matches(filter, &entry.scope_path))
            .collect(),
    };

    let all_scopes = store.list_scopes(None).await?;
    let scopes: Vec<Scope> = match &scope_filter {
        ScopeFilter::All => all_scopes,
        filter => all_scopes
            .into_iter()
            .filter(|scope| scope_filter_matches(filter, &scope.path))
            .collect(),
    };

    let count = entries.len();

    Ok(ExportView {
        entries,
        scopes,
        exported_at: Utc::now(),
        count,
    })
}

fn scope_filter_matches(filter: &ScopeFilter, path: &ScopePath) -> bool {
    match filter {
        ScopeFilter::Exact(scope_path) | ScopeFilter::AncestorWalk(scope_path) => {
            path == scope_path
        }
        ScopeFilter::Subtree(scope_path) => scope_path_contains(scope_path, path),
        ScopeFilter::Set(scope_paths) => scope_paths.iter().any(|scope_path| scope_path == path),
        ScopeFilter::All => true,
    }
}

fn scope_path_contains(root: &ScopePath, candidate: &ScopePath) -> bool {
    candidate == root
        || candidate
            .as_str()
            .strip_prefix(root.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
}
