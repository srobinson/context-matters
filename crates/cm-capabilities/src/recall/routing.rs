use cm_core::{
    AncestorWalkRequest, CmError, ContextStore, Entry, EntryFilter, EntryKind, FtsQuery,
    Pagination, ScopeFilter, ScopePath,
};

use crate::constants::MAX_LIMIT;
use crate::projection::{RecallRow, entry_has_any_tag};

use super::types::{RecallRequest, RecallRouting, SearchTier};

/// Returns `(rows, routing, actual_fetch_limit, tier)`.
///
/// The third element is the SQL LIMIT actually used in the fetch, which differs
/// from the top-level `fetch_limit` for `TagScopeWalk`, which uses `MAX_LIMIT`
/// per page. The fourth element is `Some(_)` only when `routing == Search`.
pub(super) async fn route_query(
    store: &impl ContextStore,
    request: &RecallRequest,
    scope: Option<&ScopePath>,
    fetch_limit: u32,
) -> Result<(Vec<RecallRow>, RecallRouting, u32, Option<SearchTier>), CmError> {
    match &request.query {
        Some(query) => route_search(store, query, scope, fetch_limit).await,
        None if !request.tags.is_empty() => {
            let entries = recall_candidates_without_query(
                store,
                scope,
                &request.kinds,
                &request.tags,
                fetch_limit,
            )
            .await?;
            Ok((
                wrap_scoreless(entries),
                RecallRouting::TagScopeWalk,
                MAX_LIMIT,
                None,
            ))
        }
        None => route_without_query(store, request, scope, fetch_limit).await,
    }
}

async fn route_search(
    store: &impl ContextStore,
    query: &str,
    scope: Option<&ScopePath>,
    fetch_limit: u32,
) -> Result<(Vec<RecallRow>, RecallRouting, u32, Option<SearchTier>), CmError> {
    if let Some(rows) = try_search_tier(store, FtsQuery::new(query), scope, fetch_limit).await? {
        return Ok((
            rows,
            RecallRouting::Search,
            fetch_limit,
            Some(SearchTier::Exact),
        ));
    }
    if let Some(rows) = try_search_tier(
        store,
        FtsQuery::recall_auto_prefix(query),
        scope,
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
    if let Some(rows) =
        try_search_tier(store, FtsQuery::split_or_query(query), scope, fetch_limit).await?
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

async fn route_without_query(
    store: &impl ContextStore,
    request: &RecallRequest,
    scope: Option<&ScopePath>,
    fetch_limit: u32,
) -> Result<(Vec<RecallRow>, RecallRouting, u32, Option<SearchTier>), CmError> {
    match scope {
        Some(scope_path) => {
            let entries = store
                .resolve_context(scope_path, &request.kinds, fetch_limit)
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

fn wrap_scoreless(entries: Vec<Entry>) -> Vec<RecallRow> {
    entries
        .into_iter()
        .map(|entry| RecallRow { entry, score: None })
        .collect()
}

/// Run a single FTS5 tier and return its rows if the query is non-empty and
/// the store returned at least one match.
///
/// Empty sanitized queries, zero rows, and FTS5 parse errors are treated as
/// tier misses so the cascade can continue to the next broader query shape.
async fn try_search_tier(
    store: &impl ContextStore,
    fts: FtsQuery,
    scope: Option<&ScopePath>,
    limit: u32,
) -> Result<Option<Vec<RecallRow>>, CmError> {
    if fts.as_str().is_empty() {
        return Ok(None);
    }
    let Some(scope) = scope else {
        return Ok(None);
    };
    let scored = match store
        .do_search_ancestor_walk(AncestorWalkRequest {
            query: fts.as_str().to_owned(),
            scope: scope.clone(),
            limit,
        })
        .await
    {
        Ok(rows) => rows,
        Err(err) if is_fts5_parse_error(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    if scored.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        scored
            .into_iter()
            .map(|scored_entry| RecallRow {
                entry: scored_entry.entry,
                score: Some(scored_entry.score),
            })
            .collect(),
    ))
}

fn is_fts5_parse_error(err: &CmError) -> bool {
    matches!(err, CmError::Database(msg) if msg.contains("fts5:"))
}

/// Browse through scopes and pages until enough no-query recall matches are found.
///
/// Preserves recall semantics for scoped ancestor walks while avoiding false
/// negatives from fetching one widened page and post-filtering it.
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
            .map(|path| ScopePath::parse(path).map(Some))
            .collect::<Result<_, _>>()?,
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
                    scope: scoped_path.clone().map(ScopeFilter::Exact),
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
