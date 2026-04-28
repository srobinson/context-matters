use std::collections::HashMap;

use cm_core::{BrowseSort, CmError, ContextStore, Entry, EntryFilter, EntryKind, Pagination};
use uuid::Uuid;

use crate::scope::{CWD_INFERRED_SCOPE, ScopeResolution, ScopeSelector, resolve_browse_scope};
use crate::validation::clamp_limit;

// ── Types ────────────────────────────────────────────────────────

pub const DEFAULT_BROWSE_SCOPE: &str = CWD_INFERRED_SCOPE;

pub const BROWSE_SCOPE_DEFAULT_ADVISORY: &str = "no scope specified, using scope='cwd_inferred' to infer the local scope. run `cm stats` to list all scopes.";

/// Input for a browse operation.
#[derive(Debug, Clone, Default)]
pub struct BrowseRequest {
    /// Scope selector. Defaults to `cwd_inferred` when unset.
    pub scope: Option<ScopeSelector>,
    /// Whether to render smart-scope resolution metadata. Defaults to true
    /// for effective `scope="cwd_inferred"` requests and false otherwise.
    pub include_resolution: Option<bool>,
    pub kind: Option<EntryKind>,
    pub tag: Option<String>,
    pub created_by: Option<String>,
    pub include_superseded: bool,
    pub sort: BrowseSort,
    /// Requested page size. Clamped by the capability before querying.
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// Result of a browse operation.
#[derive(Debug, Clone)]
pub struct BrowseResult {
    pub entries: Vec<Entry>,
    pub total: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    /// Effective scope input after capability defaults. Used by projection
    /// code to disclose `scope=cwd_inferred` when callers omitted a scope.
    pub scope_used: Option<String>,
    pub include_resolution: bool,
    pub limit_used: u32,
    /// Sort order actually applied to the query. Always populated (defaults
    /// to `BrowseSort::Recent` when the caller omits `sort`). The browse
    /// formatter surfaces this in the result header, e.g. `sort: recent`.
    pub sort_used: BrowseSort,
    /// Outgoing-relation counts per row id, populated by a single
    /// `ContextStore::count_relations_for` batch call after the page is
    /// fetched. Ids with zero outgoing edges are **omitted** from the map
    /// (per the trait contract); the projection layer treats absence as
    /// zero and elides the `rels:` annotation entirely.
    pub relation_counts: HashMap<Uuid, u32>,
    pub resolution: Option<ScopeResolution>,
    pub advisory: Option<String>,
}

// ── Core Function ────────────────────────────────────────────────

/// Execute a browse operation against the store.
///
/// Constructs an `EntryFilter` from the request, calls `store.browse()`,
/// and returns a domain result. Adapters handle their own projection
/// (MCP uses `BrowseEntryView`, web may use a different format).
pub async fn browse(
    store: &impl ContextStore,
    request: BrowseRequest,
) -> Result<BrowseResult, CmError> {
    let scope_defaulted = request.scope.is_none();
    let mut effective_request = request;
    if scope_defaulted {
        effective_request.scope = Some(ScopeSelector::cwd_inferred(None));
    }

    let scope_is_cwd_inferred = matches!(
        effective_request.scope,
        Some(ScopeSelector::CwdInferred { .. })
    );
    let include_resolution = effective_request
        .include_resolution
        .unwrap_or(scope_is_cwd_inferred);
    let limit_used = clamp_limit(effective_request.limit);

    // Capture the resolved sort before moving `request.sort` into the filter,
    // so the formatter can surface "sort: <variant>" in the browse header
    // without having to re-derive it from request-side state.
    let sort_used = effective_request.sort;
    let selector = effective_request
        .scope
        .as_ref()
        .expect("browse scope is defaulted before resolution");
    let scope_used = Some(selector.requested_scope());
    let resolved_scope = resolve_browse_scope(store, selector).await?;

    let filter = EntryFilter {
        scope_path: resolved_scope.scope_path,
        kind: effective_request.kind,
        tag: effective_request.tag,
        created_by: effective_request.created_by,
        include_superseded: effective_request.include_superseded,
        sort: effective_request.sort,
        pagination: Pagination {
            limit: limit_used,
            cursor: effective_request.cursor,
        },
    };

    let result = store.browse(filter).await?;
    let has_more = result.next_cursor.is_some();

    // Single batched fetch of outgoing-relation counts for the page. Runs
    // after `store.browse` returns so we never count edges for rows the
    // caller will not see; the trait short-circuits on empty input.
    let relation_count_ids: Vec<Uuid> = result.items.iter().map(|e| e.id).collect();
    let relation_counts = store.count_relations_for(&relation_count_ids).await?;

    Ok(BrowseResult {
        entries: result.items,
        total: result.total,
        next_cursor: result.next_cursor,
        has_more,
        scope_used,
        include_resolution,
        limit_used,
        sort_used,
        relation_counts,
        resolution: resolved_scope.resolution,
        advisory: scope_defaulted.then(|| BROWSE_SCOPE_DEFAULT_ADVISORY.to_owned()),
    })
}
