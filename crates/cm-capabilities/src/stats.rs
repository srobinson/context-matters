use cm_core::{CmError, ContextStore, StoreStats};

// ── Types ────────────────────────────────────────────────────────

/// How to sort the `entries_by_tag` list in the result.
#[derive(Debug, Clone, Copy, Default)]
pub enum TagSort {
    /// Alphabetical by tag name.
    #[default]
    Name,
    /// By count descending (store default ordering).
    Count,
}

/// Input for a stats operation.
#[derive(Debug, Clone, Default)]
pub struct StatsRequest {
    pub tag_sort: TagSort,
}

/// A scope with its entry count, for the scope tree.
#[derive(Debug, Clone)]
pub struct ScopeTreeNode {
    pub path: String,
    pub kind: String,
    pub label: String,
    pub entry_count: u64,
}

/// Result of a stats operation.
#[derive(Debug, Clone)]
pub struct StatsResult {
    pub stats: StoreStats,
    pub scope_tree: Vec<ScopeTreeNode>,
}

// ── Core Function ────────────────────────────────────────────────

/// Fetch aggregate stats and build the scope tree.
///
/// Calls `store.stats()` for base counters, `store.list_scopes(None)` for the
/// full scope list, then joins each scope with its entry count from
/// `stats.entries_by_scope`. Tag sorting is applied per `request.tag_sort`.
pub async fn stats(
    store: &impl ContextStore,
    request: StatsRequest,
) -> Result<StatsResult, CmError> {
    let mut base = store.stats().await?;
    let scopes = store.list_scopes(None).await?;

    let scope_tree: Vec<ScopeTreeNode> = scopes
        .iter()
        .map(|s| {
            let entry_count = base
                .entries_by_scope
                .get(s.path.as_str())
                .copied()
                .unwrap_or(0);
            ScopeTreeNode {
                path: s.path.as_str().to_owned(),
                kind: s.kind.as_str().to_owned(),
                label: s.label.clone(),
                entry_count,
            }
        })
        .collect();

    match request.tag_sort {
        TagSort::Name => base.entries_by_tag.sort_by(|a, b| a.tag.cmp(&b.tag)),
        TagSort::Count => {} // Store returns sorted by count DESC already
    }

    Ok(StatsResult {
        stats: base,
        scope_tree,
    })
}
