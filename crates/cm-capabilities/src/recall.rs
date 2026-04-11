use cm_core::{CmError, ContextStore, Entry, EntryFilter, EntryKind, Pagination, ScopePath};
use serde::{Deserialize, Serialize};

use crate::constants::MAX_LIMIT;
use crate::projection::{RecallRow, entry_has_any_tag, estimate_tokens, project_recall_entry};

// ── Types ────────────────────────────────────────────────────────

/// Input for a recall operation.
#[derive(Debug, Clone, Default)]
pub struct RecallRequest {
    pub query: Option<String>,
    pub scope: Option<ScopePath>,
    pub kinds: Vec<EntryKind>,
    pub tags: Vec<String>,
    pub limit: u32,
    pub max_tokens: Option<u32>,
}

/// Which code path was taken during recall routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallRouting {
    Search,
    TagScopeWalk,
    ScopeResolve,
    BrowseFallback,
}

/// Result of a recall operation.
#[derive(Debug, Clone)]
pub struct RecallResult {
    /// Recall rows, each pairing an `Entry` with an optional FTS5 score.
    /// Scores are populated only on the `Search` routing branch; every
    /// other branch leaves `score` as `None`.
    pub entries: Vec<RecallRow>,
    pub scope_chain: Vec<String>,
    /// Per-scope hit counts. When scope is provided, ordered from most specific
    /// ancestor to broadest. When scope is omitted, derived from returned entries
    /// (most specific first).
    pub scope_hits: Vec<(String, usize)>,
    /// Total estimated tokens across all returned entries (full body, not snippets).
    pub token_estimate: u32,
    pub routing: RecallRouting,
    pub candidates_before_filter: usize,
    pub fetch_limit_used: u32,
}

// ── Core Function ────────────────────────────────────────────────

/// Execute a recall operation against the store.
///
/// Routes to the appropriate query path based on input parameters,
/// applies post-filtering, token budget tracking, and scope chain extraction.
pub async fn recall(
    store: &impl ContextStore,
    request: RecallRequest,
) -> Result<RecallResult, CmError> {
    let has_post_filter = !request.kinds.is_empty() || !request.tags.is_empty();
    let fetch_limit = if has_post_filter {
        request.limit.saturating_mul(3).min(MAX_LIMIT)
    } else {
        request.limit
    };

    // Route to the appropriate query path. The result is already wrapped in
    // `RecallRow`: the `Search` branch populates `score`; every other branch
    // leaves it `None`.
    let (raw_rows, routing, actual_fetch_limit) = route_query(store, &request, fetch_limit).await?;
    let candidates_before_filter = raw_rows.len();

    // Post-filter by kinds. Some routing paths (ScopeResolve, TagScopeWalk)
    // already filter internally, making this a no-op for those paths.
    // BrowseFallback and Search need this when kinds.len() > 1 since
    // EntryFilter.kind only accepts a single Option<EntryKind>.
    let rows: Vec<RecallRow> = if !request.kinds.is_empty() {
        raw_rows
            .into_iter()
            .filter(|r| request.kinds.contains(&r.entry.kind))
            .collect()
    } else {
        raw_rows
    };

    // Post-filter by tags
    let rows: Vec<RecallRow> = if request.tags.is_empty() {
        rows
    } else {
        rows.into_iter()
            .filter(|r| entry_has_any_tag(&r.entry, &request.tags))
            .collect()
    };

    // Sort by scope depth descending (most specific first).
    // Stable sort preserves the store's native ordering (relevance or recency) within each scope level.
    let mut rows: Vec<RecallRow> = rows;
    rows.sort_by(|a, b| {
        b.entry
            .scope_path
            .as_str()
            .len()
            .cmp(&a.entry.scope_path.as_str().len())
    });

    // Apply limit after post-filtering and sorting
    let rows: Vec<RecallRow> = rows.into_iter().take(request.limit as usize).collect();

    // Token budget tracking
    let mut budget_rows = Vec::with_capacity(rows.len());
    let mut total_tokens: u32 = 0;

    for row in &rows {
        let view = project_recall_entry(&row.entry);
        let entry_str = serde_json::to_string(&view).unwrap_or_default();
        let entry_tokens = estimate_tokens(&entry_str);

        if let Some(budget) = request.max_tokens
            && total_tokens + entry_tokens > budget
            && !budget_rows.is_empty()
        {
            break;
        }

        total_tokens += entry_tokens;
        budget_rows.push(row.clone());
    }

    // Build scope chain and hits
    let (scope_chain, scope_hits) = match &request.scope {
        Some(sp) => {
            // Explicit scope: chain from the provided scope path
            let chain: Vec<String> = sp.ancestors().map(String::from).collect();
            let hits: Vec<(String, usize)> = chain
                .iter()
                .map(|s| {
                    let count = budget_rows
                        .iter()
                        .filter(|r| r.entry.scope_path.as_str() == s)
                        .count();
                    (s.clone(), count)
                })
                .collect();
            (chain, hits)
        }
        None => {
            // No scope provided: derive from returned entries
            let mut seen = std::collections::BTreeMap::<String, usize>::new();
            for row in &budget_rows {
                *seen
                    .entry(row.entry.scope_path.as_str().to_owned())
                    .or_default() += 1;
            }
            let mut hits: Vec<(String, usize)> = seen.into_iter().collect();
            // Sort by depth descending (most specific first), then alphabetically
            hits.sort_by(|a, b| {
                let depth_a = a.0.matches('/').count();
                let depth_b = b.0.matches('/').count();
                depth_b.cmp(&depth_a).then_with(|| a.0.cmp(&b.0))
            });
            let chain: Vec<String> = hits.iter().map(|(s, _)| s.clone()).collect();
            (chain, hits)
        }
    };

    Ok(RecallResult {
        entries: budget_rows,
        scope_chain,
        scope_hits,
        token_estimate: total_tokens,
        routing,
        candidates_before_filter,
        fetch_limit_used: actual_fetch_limit,
    })
}

// ── Routing ──────────────────────────────────────────────────────

/// Wrap a plain `Entry` iterator as score-less `RecallRow`s.
///
/// Non-search routing branches do not carry relevance scores, so every
/// resulting row has `score: None`.
fn wrap_scoreless(entries: Vec<Entry>) -> Vec<RecallRow> {
    entries
        .into_iter()
        .map(|entry| RecallRow { entry, score: None })
        .collect()
}

/// Returns `(rows, routing, actual_fetch_limit)`.
///
/// The third element is the SQL LIMIT actually used in the fetch, which differs
/// from the top-level `fetch_limit` for `TagScopeWalk` (uses `MAX_LIMIT` per page).
async fn route_query(
    store: &impl ContextStore,
    request: &RecallRequest,
    fetch_limit: u32,
) -> Result<(Vec<RecallRow>, RecallRouting, u32), CmError> {
    match &request.query {
        Some(query) => {
            let scored = store
                .search(query, request.scope.as_ref(), fetch_limit)
                .await?;
            let rows: Vec<RecallRow> = scored
                .into_iter()
                .map(|s| RecallRow {
                    entry: s.entry,
                    score: Some(s.score),
                })
                .collect();
            Ok((rows, RecallRouting::Search, fetch_limit))
        }
        None => {
            if !request.tags.is_empty() {
                let entries = recall_candidates_without_query(
                    store,
                    request.scope.as_ref(),
                    &request.kinds,
                    &request.tags,
                    request.limit,
                )
                .await?;
                Ok((
                    wrap_scoreless(entries),
                    RecallRouting::TagScopeWalk,
                    MAX_LIMIT,
                ))
            } else {
                match &request.scope {
                    Some(sp) => {
                        let entries = store
                            .resolve_context(sp, &request.kinds, fetch_limit)
                            .await?;
                        Ok((
                            wrap_scoreless(entries),
                            RecallRouting::ScopeResolve,
                            fetch_limit,
                        ))
                    }
                    None => {
                        let filter = EntryFilter {
                            kind: if request.kinds.len() == 1 {
                                Some(request.kinds[0])
                            } else {
                                None
                            },
                            pagination: Pagination {
                                limit: fetch_limit,
                                cursor: None,
                            },
                            ..Default::default()
                        };
                        let paged = store.browse(filter).await?;
                        Ok((
                            wrap_scoreless(paged.items),
                            RecallRouting::BrowseFallback,
                            fetch_limit,
                        ))
                    }
                }
            }
        }
    }
}

// ── Private Helpers ──────────────────────────────────────────────

/// Browse through scopes and pages until enough no-query recall matches are found.
///
/// Preserves recall semantics for scoped ancestor walks while avoiding
/// false negatives from fetching one widened page and post-filtering it.
async fn recall_candidates_without_query(
    store: &impl ContextStore,
    scope_path: Option<&ScopePath>,
    kind_filters: &[EntryKind],
    tags: &[String],
    limit: u32,
) -> Result<Vec<Entry>, CmError> {
    let scoped_paths: Vec<Option<ScopePath>> = match scope_path {
        Some(scope_path) => scope_path
            .ancestors()
            .map(|path| ScopePath::parse(path).expect("validated ancestor path"))
            .map(Some)
            .collect(),
        None => vec![None],
    };

    let direct_kind = if kind_filters.len() == 1 {
        Some(kind_filters[0])
    } else {
        None
    };
    let direct_tag = (tags.len() == 1).then(|| tags[0].clone());
    let mut matched = Vec::new();

    for scoped_path in scoped_paths {
        let mut cursor = None;

        loop {
            let page = store
                .browse(EntryFilter {
                    scope_path: scoped_path.clone(),
                    kind: direct_kind,
                    tag: direct_tag.clone(),
                    pagination: Pagination {
                        limit: MAX_LIMIT,
                        cursor,
                    },
                    ..Default::default()
                })
                .await?;

            for entry in page.items {
                let kind_ok = kind_filters.is_empty() || kind_filters.contains(&entry.kind);
                let tag_ok = tags.is_empty() || entry_has_any_tag(&entry, tags);

                if kind_ok && tag_ok {
                    matched.push(entry);
                    if matched.len() >= limit as usize {
                        return Ok(matched);
                    }
                }
            }

            let Some(next_cursor) = page.next_cursor else {
                break;
            };
            cursor = Some(next_cursor);
        }
    }

    Ok(matched)
}
