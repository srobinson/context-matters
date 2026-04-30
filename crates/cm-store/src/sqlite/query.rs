//! Read-only query operations: resolve_context, ancestor search, browse.

use cm_core::{
    AncestorWalkRequest, CmError, Entry, EntryFilter, EntryKind, PagedResult, ScopePath,
    ScoredEntry,
};
use sqlx::{QueryBuilder, Row, Sqlite};

use super::CmStore;
use super::cursor::{decode_cursor, encode_cursor, order_by_clause, push_cursor_condition};
use super::parse::{map_db_err, parse_entry};
use super::predicates::{
    push_kind_predicate, push_scope_filter, push_tag_predicate, push_where_prefix,
};

fn push_browse_filters(query: &mut QueryBuilder<'_, Sqlite>, filter: &EntryFilter) -> bool {
    let mut has_where = false;

    if !filter.include_superseded {
        push_where_prefix(query, &mut has_where);
        query.push("superseded_by IS NULL");
    }

    if let Some(ref scope) = filter.scope {
        push_scope_filter(query, &mut has_where, scope);
    }

    if let Some(ref kind) = filter.kind {
        push_kind_predicate(query, &mut has_where, &[*kind]);
    }

    if let Some(ref created_by) = filter.created_by {
        push_where_prefix(query, &mut has_where);
        query.push("created_by = ");
        query.push_bind(created_by.clone());
    }

    if let Some(ref tag) = filter.tag {
        push_tag_predicate(query, &mut has_where, std::slice::from_ref(tag));
    }

    has_where
}

impl CmStore {
    pub(crate) async fn do_resolve_context(
        &self,
        scope_path: &ScopePath,
        kinds: &[EntryKind],
        limit: u32,
    ) -> Result<Vec<Entry>, CmError> {
        let ancestors: Vec<&str> = scope_path.ancestors().collect();
        let pool = &self.read_pool;

        let mut all_entries = Vec::new();

        // Query each ancestor level, most specific first
        for ancestor in &ancestors {
            if all_entries.len() >= limit as usize {
                break;
            }

            let remaining = limit as usize - all_entries.len();

            let rows = if kinds.is_empty() {
                sqlx::query(
                    "SELECT * FROM entries \
                     WHERE scope_path = ? AND superseded_by IS NULL \
                     ORDER BY updated_at DESC \
                     LIMIT ?",
                )
                .bind(*ancestor)
                .bind(remaining as i64)
                .fetch_all(pool)
                .await
                .map_err(map_db_err)?
            } else {
                let mut q = QueryBuilder::<Sqlite>::new(
                    "SELECT * FROM entries \
                     WHERE scope_path = ",
                );
                q.push_bind(*ancestor);
                q.push(" AND superseded_by IS NULL AND kind IN (");
                {
                    let mut separated = q.separated(", ");
                    for k in kinds {
                        separated.push_bind(k.as_str());
                    }
                }
                q.push(") ORDER BY updated_at DESC LIMIT ");
                q.push_bind(remaining as i64);

                q.build().fetch_all(pool).await.map_err(map_db_err)?
            };

            for row in &rows {
                all_entries.push(parse_entry(row)?);
            }
        }

        Ok(all_entries)
    }

    pub(crate) async fn do_search_ancestor_walk(
        &self,
        request: AncestorWalkRequest,
    ) -> Result<Vec<ScoredEntry>, CmError> {
        let fts_query = cm_core::FtsQuery::new(&request.query);
        let fts_str = fts_query.as_str().to_owned();

        if fts_str.is_empty() {
            return Ok(Vec::new());
        }

        let pool = &self.read_pool;

        let ancestors: Vec<&str> = request.scope.ancestors().collect();
        let mut q = QueryBuilder::<Sqlite>::new(
            "SELECT e.*, f.rank AS fts_rank FROM entries e \
             JOIN entries_fts f ON e.rowid = f.rowid \
             WHERE f.entries_fts MATCH ",
        );
        q.push_bind(fts_str);
        q.push(
            " \
             AND e.superseded_by IS NULL \
             AND e.scope_path IN (",
        );
        {
            let mut separated = q.separated(", ");
            for ancestor in &ancestors {
                separated.push_bind(*ancestor);
            }
        }
        q.push(") ORDER BY f.rank LIMIT ");
        q.push_bind(request.limit as i64);

        let rows = q.build().fetch_all(pool).await.map_err(map_db_err)?;

        rows.iter()
            .map(|row| {
                let entry = parse_entry(row)?;
                let rank: f64 = row.get("fts_rank");
                Ok(ScoredEntry {
                    entry,
                    score: rank as f32,
                })
            })
            .collect()
    }

    pub(crate) async fn do_browse(
        &self,
        filter: EntryFilter,
    ) -> Result<PagedResult<Entry>, CmError> {
        let pool = &self.read_pool;
        let sort = filter.sort;
        let cursor = filter
            .pagination
            .cursor
            .as_deref()
            .map(|cursor_str| decode_cursor(cursor_str, sort))
            .transpose()?;

        // Build a separate count without cursor
        let mut count_q = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) as cnt FROM entries e");
        push_browse_filters(&mut count_q, &filter);
        let (total,): (i64,) = count_q
            .build_query_as()
            .fetch_one(pool)
            .await
            .map_err(map_db_err)?;

        // Fetch query with limit + 1 to detect next page
        let fetch_limit = filter.pagination.limit as i64 + 1;
        let order = order_by_clause(sort);
        let mut data_q = QueryBuilder::<Sqlite>::new("SELECT e.* FROM entries e");
        let mut has_where = push_browse_filters(&mut data_q, &filter);
        if let Some(ref cursor) = cursor {
            push_where_prefix(&mut data_q, &mut has_where);
            push_cursor_condition(&mut data_q, cursor, sort);
        }
        data_q.push(" ");
        data_q.push(order);
        data_q.push(" LIMIT ");
        data_q.push_bind(fetch_limit);

        let rows = data_q.build().fetch_all(pool).await.map_err(map_db_err)?;

        let has_more = rows.len() > filter.pagination.limit as usize;
        let take_count = if has_more {
            filter.pagination.limit as usize
        } else {
            rows.len()
        };

        let mut items = Vec::with_capacity(take_count);
        for row in rows.iter().take(take_count) {
            items.push(parse_entry(row)?);
        }

        let next_cursor = if has_more {
            items.last().map(|last| encode_cursor(last, sort))
        } else {
            None
        };

        Ok(PagedResult {
            items,
            total: total as u64,
            next_cursor,
        })
    }
}
