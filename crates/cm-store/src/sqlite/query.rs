//! Read-only query operations: resolve_context, search, browse.

use cm_core::{CmError, Entry, EntryFilter, EntryKind, PagedResult, ScopePath};

use super::CmStore;
use super::cursor::{append_cursor_conditions, decode_cursor, encode_cursor, order_by_clause};
use super::parse::{map_db_err, parse_entry};

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
                let kind_placeholders: Vec<&str> = kinds.iter().map(|_| "?").collect();
                let sql = format!(
                    "SELECT * FROM entries \
                     WHERE scope_path = ? AND superseded_by IS NULL \
                     AND kind IN ({}) \
                     ORDER BY updated_at DESC \
                     LIMIT ?",
                    kind_placeholders.join(", ")
                );
                let mut q = sqlx::query(&sql).bind(*ancestor);
                for k in kinds {
                    q = q.bind(k.as_str());
                }
                q.bind(remaining as i64)
                    .fetch_all(pool)
                    .await
                    .map_err(map_db_err)?
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
    ) -> Result<Vec<Entry>, CmError> {
        let fts_query = cm_core::FtsQuery::new(query);
        let fts_str = fts_query.as_str().to_owned();

        if fts_str.is_empty() {
            return Ok(Vec::new());
        }

        let pool = &self.read_pool;

        let rows = if let Some(sp) = scope_path {
            let ancestors: Vec<&str> = sp.ancestors().collect();
            let placeholders: Vec<&str> = ancestors.iter().map(|_| "?").collect();
            let sql = format!(
                "SELECT e.* FROM entries e \
                 JOIN entries_fts f ON e.rowid = f.rowid \
                 WHERE f.entries_fts MATCH ? \
                 AND e.superseded_by IS NULL \
                 AND e.scope_path IN ({}) \
                 ORDER BY f.rank \
                 LIMIT ?",
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&sql).bind(&fts_str);
            for a in &ancestors {
                q = q.bind(*a);
            }
            q.bind(limit as i64)
                .fetch_all(pool)
                .await
                .map_err(map_db_err)?
        } else {
            sqlx::query(
                "SELECT e.* FROM entries e \
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

        rows.iter().map(parse_entry).collect()
    }

    pub(crate) async fn do_browse(
        &self,
        filter: EntryFilter,
    ) -> Result<PagedResult<Entry>, CmError> {
        let pool = &self.read_pool;
        let sort = filter.sort;

        let mut conditions = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if !filter.include_superseded {
            conditions.push("superseded_by IS NULL".to_owned());
        }

        if let Some(ref sp) = filter.scope_path {
            conditions.push("scope_path = ?".to_owned());
            bind_values.push(sp.as_str().to_owned());
        }

        if let Some(ref kind) = filter.kind {
            conditions.push("kind = ?".to_owned());
            bind_values.push(kind.as_str().to_owned());
        }

        if let Some(ref created_by) = filter.created_by {
            conditions.push("created_by = ?".to_owned());
            bind_values.push(created_by.clone());
        }

        if let Some(ref tag) = filter.tag {
            // JSON contains check for tags array
            conditions.push("json_extract(meta, '$.tags') LIKE ?".to_owned());
            bind_values.push(format!("%\"{tag}\"%"));
        }

        // Cursor-based keyset pagination
        if let Some(ref cursor_str) = filter.pagination.cursor {
            let cursor = decode_cursor(cursor_str, sort)?;
            append_cursor_conditions(&cursor, sort, &mut conditions, &mut bind_values);
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Build a separate count without cursor
        let mut count_conditions = Vec::new();
        let mut count_binds: Vec<String> = Vec::new();

        if !filter.include_superseded {
            count_conditions.push("superseded_by IS NULL".to_owned());
        }
        if let Some(ref sp) = filter.scope_path {
            count_conditions.push("scope_path = ?".to_owned());
            count_binds.push(sp.as_str().to_owned());
        }
        if let Some(ref kind) = filter.kind {
            count_conditions.push("kind = ?".to_owned());
            count_binds.push(kind.as_str().to_owned());
        }
        if let Some(ref created_by) = filter.created_by {
            count_conditions.push("created_by = ?".to_owned());
            count_binds.push(created_by.clone());
        }
        if let Some(ref tag) = filter.tag {
            count_conditions.push("json_extract(meta, '$.tags') LIKE ?".to_owned());
            count_binds.push(format!("%\"{tag}\"%"));
        }

        let count_where = if count_conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", count_conditions.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) as cnt FROM entries {count_where}");
        let mut count_q = sqlx::query_as::<sqlx::Sqlite, (i64,)>(&count_sql);
        for v in &count_binds {
            count_q = count_q.bind(v);
        }
        let (total,): (i64,) = count_q.fetch_one(pool).await.map_err(map_db_err)?;

        // Fetch query with limit + 1 to detect next page
        let fetch_limit = filter.pagination.limit as i64 + 1;
        let order = order_by_clause(sort);
        let data_sql = format!("SELECT * FROM entries {where_clause} {order} LIMIT ?");
        let mut data_q = sqlx::query(&data_sql);
        for v in &bind_values {
            data_q = data_q.bind(v);
        }
        data_q = data_q.bind(fetch_limit);

        let rows = data_q.fetch_all(pool).await.map_err(map_db_err)?;

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
