//! Entry CRUD and lifecycle operations.
//!
//! Implements create, get, update, supersede, and forget on CmStore.
//! All write methods run in transactions and record mutation history.

use chrono::Utc;
use cm_core::{CmError, Entry, EntryKind, MutationAction, NewEntry, UpdateEntry, WriteContext};
use uuid::Uuid;

use crate::dedup;

use super::CmStore;
use super::mutation::{entry_snapshot, insert_mutation};
use super::parse::{map_db_err, parse_entry};

impl CmStore {
    pub(crate) async fn do_create_entry(
        &self,
        new_entry: NewEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError> {
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

        let pool = &self.write_pool;

        dedup::check_duplicate(pool, &content_hash, None).await?;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

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
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.message().contains("FOREIGN KEY constraint failed") {
                    return CmError::ScopeNotFound(scope_str.clone());
                }
            map_db_err(e)
        })?;

        // Fetch the created entry within the transaction
        let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&id_str)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_err)?;

        let entry = parse_entry(&row)?;
        let after = entry_snapshot(&entry)?;

        insert_mutation(
            &mut *tx,
            &id_str,
            MutationAction::Create,
            ctx.source,
            None,
            Some(&after),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

        Ok(entry)
    }

    pub(crate) async fn do_get_entry(&self, id: Uuid) -> Result<Entry, CmError> {
        let id_str = id.to_string();
        let pool = &self.read_pool;

        let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(pool)
            .await
            .map_err(map_db_err)?;

        match row {
            Some(r) => parse_entry(&r),
            None => Err(CmError::EntryNotFound(id)),
        }
    }

    pub(crate) async fn do_get_entries(&self, ids: &[Uuid]) -> Result<Vec<Entry>, CmError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let pool = &self.read_pool;
        let id_strs: Vec<String> = ids.iter().map(|id| id.to_string()).collect();

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
        let mut entry_map: std::collections::HashMap<String, Entry> =
            std::collections::HashMap::new();
        for row in &rows {
            let entry = parse_entry(row)?;
            entry_map.insert(entry.id.to_string(), entry);
        }

        // Return in input order, skipping missing
        Ok(id_strs
            .iter()
            .filter_map(|id_str| entry_map.remove(id_str.as_str()))
            .collect())
    }

    pub(crate) async fn do_update_entry(
        &self,
        id: Uuid,
        update: UpdateEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError> {
        // Validate non-empty title/body if provided, matching create_entry's invariant
        if let Some(ref title) = update.title
            && title.trim().is_empty()
        {
            return Err(CmError::Validation("title cannot be empty".to_owned()));
        }
        if let Some(ref body) = update.body
            && body.trim().is_empty()
        {
            return Err(CmError::Validation("body cannot be empty".to_owned()));
        }

        let id_str = id.to_string();
        let pool = &self.write_pool;

        // All reads, hash derivation, dedup checks, and writes happen inside
        // one transaction to prevent TOCTOU races where a concurrent writer
        // could modify the entry between the initial read and the UPDATE.
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

        // Fetch current entry inside transaction (before snapshot + hash source)
        let current_row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_err)?
            .ok_or(CmError::EntryNotFound(id))?;

        let current = parse_entry(&current_row)?;
        let before = entry_snapshot(&current)?;

        // Compute hash from in-transaction state
        let new_hash = dedup::recompute_hash_for_update(
            current.scope_path.as_str(),
            current.kind.as_str(),
            &current.body,
            update.kind.as_ref().map(EntryKind::as_str),
            update.body.as_deref(),
        );

        // Dedup check inside transaction
        if let Some(ref hash) = new_hash {
            dedup::check_duplicate(&mut *tx, hash, Some(&id_str)).await?;
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
            // No-op: no mutation record needed, rollback implicit on drop
            return Ok(current);
        }

        let sql = format!("UPDATE entries SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        for v in &values {
            q = q.bind(v);
        }
        q = q.bind(&id_str);
        q.execute(&mut *tx).await.map_err(map_db_err)?;

        // Fetch updated entry (after snapshot)
        let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&id_str)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_err)?;

        let entry = parse_entry(&row)?;
        let after = entry_snapshot(&entry)?;

        insert_mutation(
            &mut *tx,
            &id_str,
            MutationAction::Update,
            ctx.source,
            Some(&before),
            Some(&after),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

        Ok(entry)
    }

    pub(crate) async fn do_supersede_entry(
        &self,
        old_id: Uuid,
        new_entry: NewEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError> {
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

        // Dedup check for the new entry's content
        dedup::check_duplicate(pool, &content_hash, None).await?;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

        // Fetch old entry inside the transaction (before snapshot)
        let old_row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&old_id_str)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_err)?
            .ok_or(CmError::EntryNotFound(old_id))?;

        let old_entry = parse_entry(&old_row)?;
        let old_before = entry_snapshot(&old_entry)?;

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

        // Fetch old entry again for after snapshot (now has superseded_by set)
        let old_after_row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&old_id_str)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_err)?;

        let old_after_entry = parse_entry(&old_after_row)?;
        let old_after = entry_snapshot(&old_after_entry)?;

        // Fetch new entry for after snapshot
        let new_row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&new_id_str)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_err)?;

        let new_entry_result = parse_entry(&new_row)?;
        let new_after = entry_snapshot(&new_entry_result)?;

        // Mutation 1: old entry superseded
        insert_mutation(
            &mut *tx,
            &old_id_str,
            MutationAction::Supersede,
            ctx.source,
            Some(&old_before),
            Some(&old_after),
        )
        .await?;

        // Mutation 2: new entry created
        insert_mutation(
            &mut *tx,
            &new_id_str,
            MutationAction::Create,
            ctx.source,
            None,
            Some(&new_after),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

        Ok(new_entry_result)
    }

    pub(crate) async fn do_forget_entry(
        &self,
        id: Uuid,
        ctx: &WriteContext,
    ) -> Result<(), CmError> {
        let id_str = id.to_string();
        let pool = &self.write_pool;

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CmError::Database(e.to_string()))?;

        // Fetch current entry inside transaction (before snapshot)
        let row = sqlx::query("SELECT * FROM entries WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_err)?;

        match row {
            None => return Err(CmError::EntryNotFound(id)),
            Some(ref r) => {
                let entry = parse_entry(r)?;
                if entry.superseded_by.is_some() {
                    // Already superseded/forgotten, no-op. No mutation record.
                    return Ok(());
                }

                let before = entry_snapshot(&entry)?;

                sqlx::query(
                    "UPDATE entries SET superseded_by = ? WHERE id = ? AND superseded_by IS NULL",
                )
                .bind(&id_str)
                .bind(&id_str)
                .execute(&mut *tx)
                .await
                .map_err(map_db_err)?;

                // Fetch after-state (now has superseded_by = self)
                let after_row = sqlx::query("SELECT * FROM entries WHERE id = ?")
                    .bind(&id_str)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(map_db_err)?;

                let after_entry = parse_entry(&after_row)?;
                let after = entry_snapshot(&after_entry)?;

                insert_mutation(
                    &mut *tx,
                    &id_str,
                    MutationAction::Forget,
                    ctx.source,
                    Some(&before),
                    Some(&after),
                )
                .await?;

                tx.commit()
                    .await
                    .map_err(|e| CmError::Database(e.to_string()))?;
            }
        }

        Ok(())
    }
}
