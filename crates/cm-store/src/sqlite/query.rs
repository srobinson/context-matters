//! Read-only query operations: resolve_context, search, browse.

use cm_core::{CmError, Entry, EntryFilter, EntryKind, PagedResult, ScopePath, ScoredEntry};
use sqlx::{QueryBuilder, Row, Sqlite};

use super::CmStore;
use super::cursor::{decode_cursor, encode_cursor, order_by_clause, push_cursor_condition};
use super::parse::{map_db_err, parse_entry};

fn push_where_prefix(query: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool) {
    if *has_where {
        query.push(" AND ");
    } else {
        query.push(" WHERE ");
        *has_where = true;
    }
}

fn push_browse_filters(query: &mut QueryBuilder<'_, Sqlite>, filter: &EntryFilter) -> bool {
    let mut has_where = false;

    if !filter.include_superseded {
        push_where_prefix(query, &mut has_where);
        query.push("superseded_by IS NULL");
    }

    if let Some(ref sp) = filter.scope_path {
        push_where_prefix(query, &mut has_where);
        query.push("scope_path = ");
        query.push_bind(sp.as_str().to_owned());
    }

    if let Some(ref kind) = filter.kind {
        push_where_prefix(query, &mut has_where);
        query.push("kind = ");
        query.push_bind(kind.as_str().to_owned());
    }

    if let Some(ref created_by) = filter.created_by {
        push_where_prefix(query, &mut has_where);
        query.push("created_by = ");
        query.push_bind(created_by.clone());
    }

    if let Some(ref tag) = filter.tag {
        push_where_prefix(query, &mut has_where);
        query.push("EXISTS (SELECT 1 FROM json_each(entries.meta, '$.tags') WHERE value = ");
        query.push_bind(tag.clone());
        query.push(")");
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

    pub(crate) async fn do_search(
        &self,
        query: &str,
        scope_path: Option<&ScopePath>,
        limit: u32,
    ) -> Result<Vec<ScoredEntry>, CmError> {
        let fts_query = cm_core::FtsQuery::new(query);
        let fts_str = fts_query.as_str().to_owned();

        if fts_str.is_empty() {
            return Ok(Vec::new());
        }

        let pool = &self.read_pool;

        // Both branches select `f.rank` as the trailing column so the row
        // decoder can pair each `Entry` with its raw BM25 score. SQLite's
        // FTS5 `rank` is a negative float (lower = more relevant).
        let rows = if let Some(sp) = scope_path {
            let ancestors: Vec<&str> = sp.ancestors().collect();
            let mut q = QueryBuilder::<Sqlite>::new(
                "SELECT e.*, f.rank AS fts_rank FROM entries e \
                 JOIN entries_fts f ON e.rowid = f.rowid \
                 WHERE f.entries_fts MATCH ",
            );
            q.push_bind(fts_str.clone());
            q.push(
                " \
                 AND e.superseded_by IS NULL \
                 AND e.scope_path IN (",
            );
            {
                let mut separated = q.separated(", ");
                for a in &ancestors {
                    separated.push_bind(*a);
                }
            }
            q.push(") ORDER BY f.rank LIMIT ");
            q.push_bind(limit as i64);

            q.build().fetch_all(pool).await.map_err(map_db_err)?
        } else {
            sqlx::query(
                "SELECT e.*, f.rank AS fts_rank FROM entries e \
                 JOIN entries_fts f ON e.rowid = f.rowid \
                 WHERE f.entries_fts MATCH ? \
                 AND e.superseded_by IS NULL \
                 ORDER BY f.rank \
                 LIMIT ?",
            )
            .bind(&fts_str)
            .bind(limit as i64)
            .fetch_all(pool)
            .await
            .map_err(map_db_err)?
        };

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
        let mut count_q = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) as cnt FROM entries");
        push_browse_filters(&mut count_q, &filter);
        let (total,): (i64,) = count_q
            .build_query_as()
            .fetch_one(pool)
            .await
            .map_err(map_db_err)?;

        // Fetch query with limit + 1 to detect next page
        let fetch_limit = filter.pagination.limit as i64 + 1;
        let order = order_by_clause(sort);
        let mut data_q = QueryBuilder::<Sqlite>::new("SELECT * FROM entries");
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
