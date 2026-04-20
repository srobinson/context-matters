use std::{collections::HashMap, path::PathBuf};

use cm_core::{
    BrowseSort, CmError, ContextStore, Entry, EntryFilter, EntryKind, Pagination, ScopePath,
};
use uuid::Uuid;

use crate::scope::{BrowseScopeMode, ScopeResolution, resolve_browse_scope};

// ── Types ────────────────────────────────────────────────────────

/// Input for a browse operation.
#[derive(Debug, Clone, Default)]
pub struct BrowseRequest {
    /// Preferred scope input. Accepts "auto" for local resolution or an
    /// explicit `ScopePath` string for exact filtering.
    pub scope: Option<String>,
    /// Backward compatible exact scope filter.
    pub scope_path: Option<ScopePath>,
    pub scope_mode: BrowseScopeMode,
    pub cwd: Option<PathBuf>,
    pub include_resolution: bool,
    pub kind: Option<EntryKind>,
    pub tag: Option<String>,
    pub created_by: Option<String>,
    pub include_superseded: bool,
    pub sort: BrowseSort,
    pub limit: u32,
    pub cursor: Option<String>,
}

/// Result of a browse operation.
#[derive(Debug, Clone)]
pub struct BrowseResult {
    pub entries: Vec<Entry>,
    pub total: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
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
    // Capture the resolved sort before moving `request.sort` into the filter,
    // so the formatter can surface "sort: <variant>" in the browse header
    // without having to re-derive it from request-side state.
    let sort_used = request.sort;
    let resolved_scope = resolve_browse_scope(store, &request).await?;

    let filter = EntryFilter {
        scope_path: resolved_scope.scope_path,
        kind: request.kind,
        tag: request.tag,
        created_by: request.created_by,
        include_superseded: request.include_superseded,
        sort: request.sort,
        pagination: Pagination {
            limit: request.limit,
            cursor: request.cursor,
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
        sort_used,
        relation_counts,
        resolution: resolved_scope.resolution,
    })
}
