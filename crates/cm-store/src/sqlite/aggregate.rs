//! Aggregation and export operations: stats, export, mutation queries.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use cm_core::{
    CmError, Entry, MutationAction, MutationRecord, MutationSource, ScopePath, StoreStats, TagCount,
};
use sqlx::Row;
use uuid::Uuid;

use super::CmStore;
use super::parse::{map_db_err, parse_entry, parse_mutation};

impl CmStore {
    pub(crate) async fn do_stats(&self) -> Result<StoreStats, CmError> {
        let pool = &self.read_pool;

        let active_row =
            sqlx::query("SELECT COUNT(*) as cnt FROM entries WHERE superseded_by IS NULL")
                .fetch_one(pool)
                .await
                .map_err(map_db_err)?;
        let active_entries: i64 = active_row.get("cnt");

        let superseded_row =
            sqlx::query("SELECT COUNT(*) as cnt FROM entries WHERE superseded_by IS NOT NULL")
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

        // Breakdown by tag
        let tag_rows = sqlx::query(
            "SELECT j.value AS tag, COUNT(*) AS cnt \
             FROM entries e, json_each(json_extract(e.meta, '$.tags')) j \
             WHERE e.superseded_by IS NULL \
               AND e.meta IS NOT NULL \
             GROUP BY j.value \
             ORDER BY cnt DESC, j.value COLLATE NOCASE ASC, j.value ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(map_db_err)?;

        let entries_by_tag: Vec<TagCount> = tag_rows
            .iter()
            .map(|row| TagCount {
                tag: row.get("tag"),
                count: row.get::<i64, _>("cnt") as u64,
            })
            .collect();

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
            entries_by_tag,
            db_size_bytes: (page_count * page_size) as u64,
        })
    }

    pub(crate) async fn do_export(
        &self,
        scope_path: Option<&ScopePath>,
    ) -> Result<Vec<Entry>, CmError> {
        let pool = &self.read_pool;

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
    }

    pub(crate) async fn do_get_mutations(
        &self,
        entry_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MutationRecord>, CmError> {
        let id_str = entry_id.to_string();
        let clamped_limit = limit.clamp(1, 200);

        let rows = sqlx::query(
            "SELECT id, entry_id, action, source, timestamp, before_snapshot, after_snapshot \
             FROM mutations WHERE entry_id = ? ORDER BY timestamp DESC, id DESC LIMIT ? OFFSET ?",
        )
        .bind(&id_str)
        .bind(clamped_limit)
        .bind(offset)
        .fetch_all(&self.read_pool)
        .await
        .map_err(map_db_err)?;

        rows.iter().map(parse_mutation).collect()
    }

    pub(crate) async fn do_list_mutations(
        &self,
        entry_id: Option<Uuid>,
        action: Option<MutationAction>,
        source: Option<MutationSource>,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        limit: u32,
    ) -> Result<Vec<MutationRecord>, CmError> {
        let mut sql = String::from(
            "SELECT id, entry_id, action, source, timestamp, before_snapshot, after_snapshot \
             FROM mutations WHERE 1=1",
        );
        let mut binds: Vec<String> = Vec::new();

        if let Some(eid) = entry_id {
            sql.push_str(" AND entry_id = ?");
            binds.push(eid.to_string());
        }
        if let Some(a) = action {
            sql.push_str(" AND action = ?");
            binds.push(a.as_str().to_owned());
        }
        if let Some(s) = source {
            sql.push_str(" AND source = ?");
            binds.push(s.as_str().to_owned());
        }
        if let Some(s) = since {
            sql.push_str(" AND timestamp >= ?");
            binds.push(s.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
        }
        if let Some(u) = until {
            sql.push_str(" AND timestamp <= ?");
            binds.push(u.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
        }

        sql.push_str(" ORDER BY timestamp DESC, id DESC LIMIT ?");
        let clamped_limit = limit.clamp(1, 200);

        let mut q = sqlx::query(&sql);
        for b in &binds {
            q = q.bind(b);
        }
        q = q.bind(clamped_limit);

        let rows = q.fetch_all(&self.read_pool).await.map_err(map_db_err)?;
        rows.iter().map(parse_mutation).collect()
    }
}
