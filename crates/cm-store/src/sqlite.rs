//! SQLite implementation of the `ContextStore` trait.
//!
//! `CmStore` wraps dual sqlx connection pools (1 writer, 4 readers)
//! and exposes an async public API. All methods map directly to the
//! synchronous `ContextStore` trait contract, with sqlx handling
//! thread dispatch internally.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use cm_core::{
    CmError, ContextStore, Entry, EntryFilter, EntryKind, EntryMeta, EntryRelation, NewEntry,
    NewScope, PagedResult, PaginationCursor, RelationKind, Scope, ScopeKind, ScopePath, StoreStats,
    UpdateEntry,
};
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use uuid::Uuid;

use crate::dedup;

/// SQLite-backed context store.
///
/// Holds dual connection pools: one for writes (max 1), one for reads (max 4).
/// Construct via `CmStore::new()` after creating pools with `schema::create_pools()`.
pub struct CmStore {
    write_pool: SqlitePool,
    read_pool: SqlitePool,
}

impl CmStore {
    /// Create a new store from pre-configured pools.
    pub fn new(write_pool: SqlitePool, read_pool: SqlitePool) -> Self {
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Access the write pool (for migrations, WAL checkpoint, etc.).
    pub fn write_pool(&self) -> &SqlitePool {
        &self.write_pool
    }

    /// Access the read pool.
    pub fn read_pool(&self) -> &SqlitePool {
        &self.read_pool
    }
}

// ── Row parsing helpers ────────────────────────────────────────────

fn parse_entry(row: &sqlx::sqlite::SqliteRow) -> Result<Entry, CmError> {
    let id_str: String = row.get("id");
    let id =
        Uuid::parse_str(&id_str).map_err(|e| CmError::Internal(format!("invalid UUID: {e}")))?;

    let scope_str: String = row.get("scope_path");
    let scope_path = ScopePath::parse(&scope_str)?;

    let kind_str: String = row.get("kind");
    let kind: EntryKind = kind_str.parse()?;

    let meta_str: Option<String> = row.get("meta");
    let meta: Option<EntryMeta> = meta_str.as_deref().map(serde_json::from_str).transpose()?;

    let created_at_str: String = row.get("created_at");
    let created_at = parse_datetime(&created_at_str)?;

    let updated_at_str: String = row.get("updated_at");
    let updated_at = parse_datetime(&updated_at_str)?;

    let superseded_str: Option<String> = row.get("superseded_by");
    let superseded_by = superseded_str
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|e| CmError::Internal(format!("invalid superseded_by UUID: {e}")))?;

    Ok(Entry {
        id,
        scope_path,
        kind,
        title: row.get("title"),
        body: row.get("body"),
        content_hash: row.get("content_hash"),
        meta,
        created_by: row.get("created_by"),
        created_at,
        updated_at,
        superseded_by,
    })
}

fn parse_scope(row: &sqlx::sqlite::SqliteRow) -> Result<Scope, CmError> {
    let path_str: String = row.get("path");
    let path = ScopePath::parse(&path_str)?;

    let kind_str: String = row.get("kind");
    let kind = match kind_str.as_str() {
        "global" => ScopeKind::Global,
        "project" => ScopeKind::Project,
        "repo" => ScopeKind::Repo,
        "session" => ScopeKind::Session,
        other => return Err(CmError::Internal(format!("invalid scope kind: {other}"))),
    };

    let parent_str: Option<String> = row.get("parent_path");
    let parent_path = parent_str.as_deref().map(ScopePath::parse).transpose()?;

    let meta_str: Option<String> = row.get("meta");
    let meta: Option<serde_json::Value> =
        meta_str.as_deref().map(serde_json::from_str).transpose()?;

    let created_at_str: String = row.get("created_at");
    let created_at = parse_datetime(&created_at_str)?;

    Ok(Scope {
        path,
        kind,
        label: row.get("label"),
        parent_path,
        meta,
        created_at,
    })
}

fn parse_relation(row: &sqlx::sqlite::SqliteRow) -> Result<EntryRelation, CmError> {
    let source_str: String = row.get("source_id");
    let source_id = Uuid::parse_str(&source_str)
        .map_err(|e| CmError::Internal(format!("invalid source UUID: {e}")))?;

    let target_str: String = row.get("target_id");
    let target_id = Uuid::parse_str(&target_str)
        .map_err(|e| CmError::Internal(format!("invalid target UUID: {e}")))?;

    let rel_str: String = row.get("relation");
    let relation: RelationKind = rel_str
        .parse()
        .map_err(|_| CmError::InvalidRelationKind(rel_str))?;

    let created_at_str: String = row.get("created_at");
    let created_at = parse_datetime(&created_at_str)?;

    Ok(EntryRelation {
        source_id,
        target_id,
        relation,
        created_at,
    })
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, CmError> {
    // SQLite strftime produces "YYYY-MM-DDTHH:MM:SS.fffZ"
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Fallback: try parsing without timezone suffix
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
                .map(|ndt| ndt.and_utc())
        })
        .map_err(|e| CmError::Internal(format!("invalid datetime '{s}': {e}")))
}

fn map_db_err(e: sqlx::Error) -> CmError {
    if let sqlx::Error::Database(ref db_err) = e {
        let msg = db_err.message();
        if msg.contains("FOREIGN KEY constraint failed") {
            return CmError::ConstraintViolation(msg.to_owned());
        }
        if msg.contains("UNIQUE constraint failed") {
            return CmError::ConstraintViolation(msg.to_owned());
        }
    }
    CmError::Database(e.to_string())
}

// ── ContextStore implementation ────────────────────────────────────

impl ContextStore for CmStore {
    fn create_entry(&self, new_entry: NewEntry) -> Result<Entry, CmError> {
        if new_entry.title.trim().is_empty() {
            return Err(CmError::Validation("title cannot be empty".to_owned()));
        }
        if new_entry.body.trim().is_empty() {
            return Err(CmError::Validation("body cannot be empty".to_owned()));
        }

        let id = Uuid::now_v7();
        let content_hash = new_entry.content_hash();
        let meta_json = new_entry
            .meta
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let scope_str = new_entry.scope_path.as_str().to_owned();
        let kind_str = new_entry.kind.as_str().to_owned();
        let id_str = id.to_string();

        // Block on async operations via the tokio handle
        let pool = &self.write_pool;

        // Check dedup
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                dedup::check_duplicate(pool, &content_hash, None).await?;

                let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

                sqlx::query(
                    "INSERT INTO entries (id, scope_path, kind, title, body, content_hash, meta, created_by, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&id_str)
                .bind(&scope_str)
                .bind(&kind_str)
                .bind(&new_entry.title)
                .bind(&new_entry.body)
                .bind(&content_hash)
                .bind(&meta_json)
                .bind(&new_entry.created_by)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map_err(|e| {
                    if let sqlx::Error::Database(ref db_err) = e
                        && db_err.message().contains("FOREIGN KEY constraint failed") {
                            return CmError::ScopeNotFound(scope_str.clone());
                        }
                    map_db_err(e)
                })?;

                // Fetch and return the created entry
                let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
                    .bind(&id_str)
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;

                parse_entry(&row)
            })
        })
    }

    fn get_entry(&self, id: Uuid) -> Result<Entry, CmError> {
        let id_str = id.to_string();
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
                    .bind(&id_str)
                    .fetch_optional(pool)
                    .await
                    .map_err(map_db_err)?;

                match row {
                    Some(r) => parse_entry(&r),
                    None => Err(CmError::EntryNotFound(id)),
                }
            })
        })
    }

    fn get_entries(&self, ids: &[Uuid]) -> Result<Vec<Entry>, CmError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let pool = &self.read_pool;
        let id_strs: Vec<String> = ids.iter().map(|id| id.to_string()).collect();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Build IN clause dynamically
                let placeholders: Vec<&str> = id_strs.iter().map(|_| "?").collect();
                let sql = format!(
                    "SELECT * FROM entries WHERE id IN ({})",
                    placeholders.join(", ")
                );

                let mut query = sqlx::query(&sql);
                for id_str in &id_strs {
                    query = query.bind(id_str);
                }

                let rows = query.fetch_all(pool).await.map_err(map_db_err)?;

                // Build a map for ordering
                let mut entry_map: HashMap<String, Entry> = HashMap::new();
                for row in &rows {
                    let entry = parse_entry(row)?;
                    entry_map.insert(entry.id.to_string(), entry);
                }

                // Return in input order, skipping missing
                Ok(id_strs
                    .iter()
                    .filter_map(|id_str| entry_map.remove(id_str.as_str()))
                    .collect())
            })
        })
    }

    fn resolve_context(
        &self,
        scope_path: &ScopePath,
        kinds: &[EntryKind],
        limit: u32,
    ) -> Result<Vec<Entry>, CmError> {
        let ancestors: Vec<&str> = scope_path.ancestors().collect();
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
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
            })
        })
    }

    fn search(
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

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
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
            })
        })
    }

    fn browse(&self, filter: EntryFilter) -> Result<PagedResult<Entry>, CmError> {
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
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

                // Cursor-based pagination
                if let Some(ref cursor) = filter.pagination.cursor {
                    conditions.push("(updated_at < ? OR (updated_at = ? AND id < ?))".to_owned());
                    let ts = cursor
                        .updated_at
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string();
                    bind_values.push(ts.clone());
                    bind_values.push(ts);
                    bind_values.push(cursor.id.to_string());
                }

                let where_clause = if conditions.is_empty() {
                    String::new()
                } else {
                    format!("WHERE {}", conditions.join(" AND "))
                };

                // For count, we don't include cursor conditions
                // Actually we do include them for consistency, but total should be without cursor
                // Let's build a separate count without cursor
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
                let data_sql = format!(
                    "SELECT * FROM entries {where_clause} \
                     ORDER BY updated_at DESC, id DESC \
                     LIMIT ?",
                );
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
                    items.last().map(|last| PaginationCursor {
                        updated_at: last.updated_at,
                        id: last.id,
                    })
                } else {
                    None
                };

                Ok(PagedResult {
                    items,
                    total: total as u64,
                    next_cursor,
                })
            })
        })
    }

    fn update_entry(&self, id: Uuid, update: UpdateEntry) -> Result<Entry, CmError> {
        // Validate non-empty title/body if provided, matching create_entry's invariant
        if let Some(ref title) = update.title
            && title.trim().is_empty() {
                return Err(CmError::Validation("title cannot be empty".to_owned()));
            }
        if let Some(ref body) = update.body
            && body.trim().is_empty() {
                return Err(CmError::Validation("body cannot be empty".to_owned()));
            }

        let id_str = id.to_string();
        let pool = &self.write_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Fetch current entry
                let current_row = sqlx::query("SELECT * FROM entries WHERE id = ?")
                    .bind(&id_str)
                    .fetch_optional(pool)
                    .await
                    .map_err(map_db_err)?
                    .ok_or(CmError::EntryNotFound(id))?;

                let current = parse_entry(&current_row)?;

                // Check dedup if body or kind changed
                let new_hash = dedup::recompute_hash_for_update(
                    current.scope_path.as_str(),
                    current.kind.as_str(),
                    &current.body,
                    update.kind.as_ref().map(EntryKind::as_str),
                    update.body.as_deref(),
                );

                if let Some(ref hash) = new_hash {
                    dedup::check_duplicate(pool, hash, Some(&id_str)).await?;
                }

                // Build dynamic UPDATE
                let mut sets = Vec::new();
                let mut values: Vec<String> = Vec::new();

                if let Some(ref title) = update.title {
                    sets.push("title = ?");
                    values.push(title.clone());
                }
                if let Some(ref body) = update.body {
                    sets.push("body = ?");
                    values.push(body.clone());
                }
                if let Some(ref kind) = update.kind {
                    sets.push("kind = ?");
                    values.push(kind.as_str().to_owned());
                }
                if let Some(ref meta) = update.meta {
                    sets.push("meta = ?");
                    values.push(serde_json::to_string(meta)?);
                }
                if let Some(ref hash) = new_hash {
                    sets.push("content_hash = ?");
                    values.push(hash.clone());
                }

                if sets.is_empty() {
                    return Ok(current);
                }

                let sql = format!("UPDATE entries SET {} WHERE id = ?", sets.join(", "));
                let mut q = sqlx::query(&sql);
                for v in &values {
                    q = q.bind(v);
                }
                q = q.bind(&id_str);
                q.execute(pool).await.map_err(map_db_err)?;

                // Fetch updated entry
                let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
                    .bind(&id_str)
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;

                parse_entry(&row)
            })
        })
    }

    fn supersede_entry(&self, old_id: Uuid, new_entry: NewEntry) -> Result<Entry, CmError> {
        // Validate upfront, matching create_entry's contract
        if new_entry.title.trim().is_empty() {
            return Err(CmError::Validation("title cannot be empty".to_owned()));
        }
        if new_entry.body.trim().is_empty() {
            return Err(CmError::Validation("body cannot be empty".to_owned()));
        }

        let old_id_str = old_id.to_string();
        let new_id = Uuid::now_v7();
        let content_hash = new_entry.content_hash();
        let meta_json = new_entry
            .meta
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let scope_str = new_entry.scope_path.as_str().to_owned();
        let kind_str = new_entry.kind.as_str().to_owned();
        let new_id_str = new_id.to_string();
        let pool = &self.write_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Verify old entry exists
                let exists = sqlx::query("SELECT id FROM entries WHERE id = ?")
                    .bind(&old_id_str)
                    .fetch_optional(pool)
                    .await
                    .map_err(map_db_err)?;

                if exists.is_none() {
                    return Err(CmError::EntryNotFound(old_id));
                }

                // Dedup check for the new entry's content
                dedup::check_duplicate(pool, &content_hash, None).await?;

                let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

                // Wrap all three mutations in a transaction for atomicity
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| CmError::Database(e.to_string()))?;

                // Insert the new entry
                sqlx::query(
                    "INSERT INTO entries (id, scope_path, kind, title, body, content_hash, meta, created_by, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&new_id_str)
                .bind(&scope_str)
                .bind(&kind_str)
                .bind(&new_entry.title)
                .bind(&new_entry.body)
                .bind(&content_hash)
                .bind(&meta_json)
                .bind(&new_entry.created_by)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    if let sqlx::Error::Database(ref db_err) = e
                        && db_err.message().contains("FOREIGN KEY constraint failed")
                    {
                        return CmError::ScopeNotFound(scope_str.clone());
                    }
                    map_db_err(e)
                })?;

                // Mark old entry as superseded
                sqlx::query("UPDATE entries SET superseded_by = ? WHERE id = ?")
                    .bind(&new_id_str)
                    .bind(&old_id_str)
                    .execute(&mut *tx)
                    .await
                    .map_err(map_db_err)?;

                // Create supersedes relation
                sqlx::query(
                    "INSERT INTO entry_relations (source_id, target_id, relation) VALUES (?, ?, 'supersedes')",
                )
                .bind(&new_id_str)
                .bind(&old_id_str)
                .execute(&mut *tx)
                .await
                .map_err(map_db_err)?;

                tx.commit()
                    .await
                    .map_err(|e| CmError::Database(e.to_string()))?;

                // Fetch the created entry (outside transaction, already committed)
                let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
                    .bind(&new_id_str)
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;

                parse_entry(&row)
            })
        })
    }

    fn forget_entry(&self, id: Uuid) -> Result<(), CmError> {
        let id_str = id.to_string();
        let pool = &self.write_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let result = sqlx::query(
                    "UPDATE entries SET superseded_by = ? WHERE id = ? AND superseded_by IS NULL",
                )
                .bind(&id_str)
                .bind(&id_str)
                .execute(pool)
                .await
                .map_err(map_db_err)?;

                if result.rows_affected() == 0 {
                    // Check if entry exists at all
                    let exists = sqlx::query("SELECT id FROM entries WHERE id = ?")
                        .bind(&id_str)
                        .fetch_optional(pool)
                        .await
                        .map_err(map_db_err)?;

                    if exists.is_none() {
                        return Err(CmError::EntryNotFound(id));
                    }
                    // Entry exists but already superseded, that is fine
                }

                Ok(())
            })
        })
    }

    fn create_relation(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        relation: RelationKind,
    ) -> Result<EntryRelation, CmError> {
        let source_str = source_id.to_string();
        let target_str = target_id.to_string();
        let rel_str = relation.as_str();
        let pool = &self.write_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query(
                    "INSERT INTO entry_relations (source_id, target_id, relation) VALUES (?, ?, ?)",
                )
                .bind(&source_str)
                .bind(&target_str)
                .bind(rel_str)
                .execute(pool)
                .await
                .map_err(|e| {
                    if let sqlx::Error::Database(ref db_err) = e {
                        let msg = db_err.message();
                        if msg.contains("FOREIGN KEY constraint failed") {
                            return CmError::EntryNotFound(source_id);
                        }
                        if msg.contains("UNIQUE constraint failed")
                            || msg.contains("PRIMARY KEY")
                        {
                            return CmError::ConstraintViolation(
                                "relation already exists".to_owned(),
                            );
                        }
                    }
                    map_db_err(e)
                })?;

                let row = sqlx::query(
                    "SELECT * FROM entry_relations WHERE source_id = ? AND target_id = ? AND relation = ?",
                )
                .bind(&source_str)
                .bind(&target_str)
                .bind(rel_str)
                .fetch_one(pool)
                .await
                .map_err(map_db_err)?;

                parse_relation(&row)
            })
        })
    }

    fn get_relations_from(&self, source_id: Uuid) -> Result<Vec<EntryRelation>, CmError> {
        let source_str = source_id.to_string();
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let rows = sqlx::query("SELECT * FROM entry_relations WHERE source_id = ?")
                    .bind(&source_str)
                    .fetch_all(pool)
                    .await
                    .map_err(map_db_err)?;

                rows.iter().map(parse_relation).collect()
            })
        })
    }

    fn get_relations_to(&self, target_id: Uuid) -> Result<Vec<EntryRelation>, CmError> {
        let target_str = target_id.to_string();
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let rows = sqlx::query("SELECT * FROM entry_relations WHERE target_id = ?")
                    .bind(&target_str)
                    .fetch_all(pool)
                    .await
                    .map_err(map_db_err)?;

                rows.iter().map(parse_relation).collect()
            })
        })
    }

    fn create_scope(&self, new_scope: NewScope) -> Result<Scope, CmError> {
        let path_str = new_scope.path.as_str().to_owned();
        let kind_str = new_scope.kind().as_str().to_owned();
        let parent = new_scope.parent_path();
        let parent_str = parent.as_ref().map(|p| p.as_str().to_owned());
        let meta_json = new_scope
            .meta
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| CmError::Internal(e.to_string()))?;
        let pool = &self.write_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                sqlx::query(
                    "INSERT INTO scopes (path, kind, label, parent_path, meta) VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&path_str)
                .bind(&kind_str)
                .bind(&new_scope.label)
                .bind(&parent_str)
                .bind(&meta_json)
                .execute(pool)
                .await
                .map_err(|e| {
                    if let sqlx::Error::Database(ref db_err) = e {
                        let msg = db_err.message();
                        if msg.contains("FOREIGN KEY constraint failed")
                            && let Some(ref p) = parent_str {
                                return CmError::ScopeNotFound(p.clone());
                            }
                        if msg.contains("UNIQUE constraint failed") || msg.contains("PRIMARY KEY") {
                            return CmError::ConstraintViolation(format!(
                                "scope already exists: {path_str}"
                            ));
                        }
                    }
                    map_db_err(e)
                })?;

                let row = sqlx::query("SELECT * FROM scopes WHERE path = ?")
                    .bind(&path_str)
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;

                parse_scope(&row)
            })
        })
    }

    fn get_scope(&self, path: &ScopePath) -> Result<Scope, CmError> {
        let path_str = path.as_str().to_owned();
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let row = sqlx::query("SELECT * FROM scopes WHERE path = ?")
                    .bind(&path_str)
                    .fetch_optional(pool)
                    .await
                    .map_err(map_db_err)?;

                match row {
                    Some(r) => parse_scope(&r),
                    None => Err(CmError::ScopeNotFound(path_str)),
                }
            })
        })
    }

    fn list_scopes(&self, kind: Option<ScopeKind>) -> Result<Vec<Scope>, CmError> {
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let rows = if let Some(k) = kind {
                    sqlx::query("SELECT * FROM scopes WHERE kind = ? ORDER BY path")
                        .bind(k.as_str())
                        .fetch_all(pool)
                        .await
                        .map_err(map_db_err)?
                } else {
                    sqlx::query("SELECT * FROM scopes ORDER BY path")
                        .fetch_all(pool)
                        .await
                        .map_err(map_db_err)?
                };

                rows.iter().map(parse_scope).collect()
            })
        })
    }

    fn stats(&self) -> Result<StoreStats, CmError> {
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let active_row =
                    sqlx::query("SELECT COUNT(*) as cnt FROM entries WHERE superseded_by IS NULL")
                        .fetch_one(pool)
                        .await
                        .map_err(map_db_err)?;
                let active_entries: i64 = active_row.get("cnt");

                let superseded_row = sqlx::query(
                    "SELECT COUNT(*) as cnt FROM entries WHERE superseded_by IS NOT NULL",
                )
                .fetch_one(pool)
                .await
                .map_err(map_db_err)?;
                let superseded_entries: i64 = superseded_row.get("cnt");

                let scopes_row = sqlx::query("SELECT COUNT(*) as cnt FROM scopes")
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;
                let scopes: i64 = scopes_row.get("cnt");

                let relations_row = sqlx::query("SELECT COUNT(*) as cnt FROM entry_relations")
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;
                let relations: i64 = relations_row.get("cnt");

                // Breakdown by kind
                let kind_rows = sqlx::query(
                    "SELECT kind, COUNT(*) as cnt FROM entries \
                     WHERE superseded_by IS NULL GROUP BY kind",
                )
                .fetch_all(pool)
                .await
                .map_err(map_db_err)?;

                let mut entries_by_kind = HashMap::new();
                for row in &kind_rows {
                    let kind: String = row.get("kind");
                    let cnt: i64 = row.get("cnt");
                    entries_by_kind.insert(kind, cnt as u64);
                }

                // Breakdown by scope
                let scope_rows = sqlx::query(
                    "SELECT scope_path, COUNT(*) as cnt FROM entries \
                     WHERE superseded_by IS NULL GROUP BY scope_path",
                )
                .fetch_all(pool)
                .await
                .map_err(map_db_err)?;

                let mut entries_by_scope = HashMap::new();
                for row in &scope_rows {
                    let sp: String = row.get("scope_path");
                    let cnt: i64 = row.get("cnt");
                    entries_by_scope.insert(sp, cnt as u64);
                }

                // DB file size (0 for in-memory)
                let page_count_row = sqlx::query("PRAGMA page_count")
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;
                let page_count: i64 = page_count_row.get(0);

                let page_size_row = sqlx::query("PRAGMA page_size")
                    .fetch_one(pool)
                    .await
                    .map_err(map_db_err)?;
                let page_size: i64 = page_size_row.get(0);

                Ok(StoreStats {
                    active_entries: active_entries as u64,
                    superseded_entries: superseded_entries as u64,
                    scopes: scopes as u64,
                    relations: relations as u64,
                    entries_by_kind,
                    entries_by_scope,
                    db_size_bytes: (page_count * page_size) as u64,
                })
            })
        })
    }

    fn export(&self, scope_path: Option<&ScopePath>) -> Result<Vec<Entry>, CmError> {
        let pool = &self.read_pool;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let rows = if let Some(sp) = scope_path {
                    sqlx::query(
                        "SELECT * FROM entries \
                         WHERE superseded_by IS NULL AND scope_path = ? \
                         ORDER BY scope_path ASC, created_at ASC",
                    )
                    .bind(sp.as_str())
                    .fetch_all(pool)
                    .await
                    .map_err(map_db_err)?
                } else {
                    sqlx::query(
                        "SELECT * FROM entries \
                         WHERE superseded_by IS NULL \
                         ORDER BY scope_path ASC, created_at ASC",
                    )
                    .fetch_all(pool)
                    .await
                    .map_err(map_db_err)?
                };

                rows.iter().map(parse_entry).collect()
            })
        })
    }
}
