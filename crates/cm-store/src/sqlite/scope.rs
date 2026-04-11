//! Scope and relation operations.

use std::collections::HashMap;

use cm_core::{
    CmError, EntryRelation, NewScope, RelationKind, Scope, ScopeKind, ScopePath, WriteContext,
};
use sqlx::Row;
use uuid::Uuid;

use super::CmStore;
use super::parse::{map_db_err, parse_relation, parse_scope};

impl CmStore {
    pub(crate) async fn do_create_relation(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        relation: RelationKind,
        _ctx: &WriteContext,
    ) -> Result<EntryRelation, CmError> {
        let source_str = source_id.to_string();
        let target_str = target_id.to_string();
        let rel_str = relation.as_str();
        let pool = &self.write_pool;

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
                if msg.contains("UNIQUE constraint failed") || msg.contains("PRIMARY KEY") {
                    return CmError::ConstraintViolation("relation already exists".to_owned());
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
    }

    pub(crate) async fn do_get_relations_from(
        &self,
        source_id: Uuid,
    ) -> Result<Vec<EntryRelation>, CmError> {
        let source_str = source_id.to_string();
        let pool = &self.read_pool;

        let rows = sqlx::query("SELECT * FROM entry_relations WHERE source_id = ?")
            .bind(&source_str)
            .fetch_all(pool)
            .await
            .map_err(map_db_err)?;

        rows.iter().map(parse_relation).collect()
    }

    pub(crate) async fn do_get_relations_to(
        &self,
        target_id: Uuid,
    ) -> Result<Vec<EntryRelation>, CmError> {
        let target_str = target_id.to_string();
        let pool = &self.read_pool;

        let rows = sqlx::query("SELECT * FROM entry_relations WHERE target_id = ?")
            .bind(&target_str)
            .fetch_all(pool)
            .await
            .map_err(map_db_err)?;

        rows.iter().map(parse_relation).collect()
    }

    /// Count outgoing relations for each id in `ids` using a single batched
    /// `IN (?, ?, ...)` query. Empty input short-circuits without touching
    /// the pool. Ids with zero outgoing relations are omitted from the map
    /// (`GROUP BY source_id` only emits rows for ids that have at least one
    /// matching relation).
    pub(crate) async fn do_count_relations_for(
        &self,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, u32>, CmError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let pool = &self.read_pool;
        let id_strs: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let placeholders: Vec<&str> = id_strs.iter().map(|_| "?").collect();
        // Single SELECT, no semicolons, one round-trip. The PRIMARY KEY on
        // (source_id, target_id, relation) makes the IN-clause lookup an
        // index-only scan.
        let sql = format!(
            "SELECT source_id, COUNT(*) AS cnt FROM entry_relations \
             WHERE source_id IN ({}) \
             GROUP BY source_id",
            placeholders.join(", ")
        );

        let mut q = sqlx::query(&sql);
        for s in &id_strs {
            q = q.bind(s);
        }
        let rows = q.fetch_all(pool).await.map_err(map_db_err)?;

        let mut counts = HashMap::with_capacity(rows.len());
        for row in &rows {
            let source_str: String = row.get("source_id");
            let cnt: i64 = row.get("cnt");
            let id = Uuid::parse_str(&source_str)
                .map_err(|e| CmError::Internal(format!("invalid uuid in entry_relations: {e}")))?;
            counts.insert(id, cnt as u32);
        }
        Ok(counts)
    }

    pub(crate) async fn do_create_scope(
        &self,
        new_scope: NewScope,
        _ctx: &WriteContext,
    ) -> Result<Scope, CmError> {
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
                    && let Some(ref p) = parent_str
                {
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
    }

    pub(crate) async fn do_get_scope(&self, path: &ScopePath) -> Result<Scope, CmError> {
        let path_str = path.as_str().to_owned();
        let pool = &self.read_pool;

        let row = sqlx::query("SELECT * FROM scopes WHERE path = ?")
            .bind(&path_str)
            .fetch_optional(pool)
            .await
            .map_err(map_db_err)?;

        match row {
            Some(r) => parse_scope(&r),
            None => Err(CmError::ScopeNotFound(path_str)),
        }
    }

    pub(crate) async fn do_list_scopes(
        &self,
        kind: Option<ScopeKind>,
    ) -> Result<Vec<Scope>, CmError> {
        let pool = &self.read_pool;

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
    }
}
