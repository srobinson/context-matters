use cm_core::{
    BrowseSort, CmError, ContextStore, Entry, EntryFilter, EntryKind, Pagination, ScopePath,
};

// ── Types ────────────────────────────────────────────────────────

/// Input for a browse operation.
#[derive(Debug, Clone, Default)]
pub struct BrowseRequest {
    pub scope_path: Option<ScopePath>,
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
    let filter = EntryFilter {
        scope_path: request.scope_path,
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

    Ok(BrowseResult {
        entries: result.items,
        total: result.total,
        next_cursor: result.next_cursor,
        has_more,
    })
}
