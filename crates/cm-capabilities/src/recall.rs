use cm_core::{
    CmError, ContextStore, Entry, EntryFilter, EntryKind, FtsQuery, Pagination, ScopePath,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::constants::MAX_LIMIT;
use crate::projection::{RecallRow, entry_has_any_tag, estimate_tokens};

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

/// Which tier of the FTS5 fallback cascade produced the returned rows.
///
/// The recall cascade tries progressively broader query shapes until one
/// returns a non-empty row set. `Exact` is the strictest (implicit AND on
/// raw tokens), `Prefix` relaxes each token to a prefix match, and
/// `SplitOr` joins tokens with `OR` so any shared term will hit. `None` is
/// reserved for the case where all three tiers were tried and none
/// returned rows (distinct from `RecallResult.tier == None`, which signals
/// the cascade was never entered because the routing was not `Search`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum SearchTier {
    Exact,
    Prefix,
    SplitOr,
    None,
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
    /// Which tier of the FTS5 cascade produced the rows. `Some(_)` only when
    /// `routing == Search`; non-search routings leave this as `None`.
    pub tier: Option<SearchTier>,
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
    let (raw_rows, routing, actual_fetch_limit, tier) =
        route_query(store, &request, fetch_limit).await?;
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

    // Token budget tracking.
    //
    // The estimate is derived directly from the body byte length (chars/4),
    // matching `project_recall_entry`'s `token_estimate`. Previously this
    // loop projected every row to a `RecallEntryView`, serialised the view
    // to a JSON string, and fed the string back to `estimate_tokens` — two
    // redundant string copies per row for an estimate the raw body already
    // provides.
    let mut budget_rows = Vec::with_capacity(rows.len());
    let mut total_tokens: u32 = 0;

    for row in &rows {
        let entry_tokens = estimate_tokens(&row.entry.body);

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
        tier,
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

/// Returns `(rows, routing, actual_fetch_limit, tier)`.
///
/// The third element is the SQL LIMIT actually used in the fetch, which differs
/// from the top-level `fetch_limit` for `TagScopeWalk` (uses `MAX_LIMIT` per page).
///
/// The fourth element is `Some(_)` only when `routing == Search`, identifying
/// which tier of the FTS5 cascade produced the rows. Non-search routings
/// return `None`.
async fn route_query(
    store: &impl ContextStore,
    request: &RecallRequest,
    fetch_limit: u32,
) -> Result<(Vec<RecallRow>, RecallRouting, u32, Option<SearchTier>), CmError> {
    match &request.query {
        Some(query) => {
            // Tiered FTS5 cascade: try progressively broader query shapes and
            // return the first non-empty tier. If every tier comes up empty,
            // return zero rows with `tier = Some(SearchTier::None)` so the
            // caller can surface the exhausted-cascade state.
            if let Some(rows) = try_search_tier(
                store,
                FtsQuery::new(query),
                request.scope.as_ref(),
                fetch_limit,
            )
            .await?
            {
                return Ok((
                    rows,
                    RecallRouting::Search,
                    fetch_limit,
                    Some(SearchTier::Exact),
                ));
            }
            if let Some(rows) = try_search_tier(
                store,
                FtsQuery::prefix_query(query),
                request.scope.as_ref(),
                fetch_limit,
            )
            .await?
            {
                return Ok((
                    rows,
                    RecallRouting::Search,
                    fetch_limit,
                    Some(SearchTier::Prefix),
                ));
            }
            if let Some(rows) = try_search_tier(
                store,
                FtsQuery::split_or_query(query),
                request.scope.as_ref(),
                fetch_limit,
            )
            .await?
            {
                return Ok((
                    rows,
                    RecallRouting::Search,
                    fetch_limit,
                    Some(SearchTier::SplitOr),
                ));
            }
            Ok((
                Vec::new(),
                RecallRouting::Search,
                fetch_limit,
                Some(SearchTier::None),
            ))
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
                    None,
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
                            None,
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
                            None,
                        ))
                    }
                }
            }
        }
    }
}

/// Run a single FTS5 tier and return its rows if the query is non-empty and
/// the store returned at least one match.
///
/// Returns `Ok(None)` both when the sanitized query is empty (so we would
/// otherwise hand FTS5 a bare empty MATCH expression) and when the store
/// returned zero rows, letting the cascade advance cleanly to the next tier.
async fn try_search_tier(
    store: &impl ContextStore,
    fts: FtsQuery,
    scope: Option<&ScopePath>,
    limit: u32,
) -> Result<Option<Vec<RecallRow>>, CmError> {
    if fts.as_str().is_empty() {
        return Ok(None);
    }
    let scored = store.search(fts.as_str(), scope, limit).await?;
    if scored.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        scored
            .into_iter()
            .map(|s| RecallRow {
                entry: s.entry,
                score: Some(s.score),
            })
            .collect(),
    ))
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
