//! Row parsing helpers for SQLite result sets.
//!
//! Pure functions that convert `SqliteRow` into domain types.
//! No I/O, no pool access. Shared across all query modules.

use chrono::{DateTime, Utc};
use cm_core::{
    CmError, Entry, EntryKind, EntryMeta, EntryRelation, MutationRecord, RelationKind, Scope,
    ScopeKind, ScopePath,
};
use sqlx::Row;
use uuid::Uuid;

pub(crate) fn parse_entry(row: &sqlx::sqlite::SqliteRow) -> Result<Entry, CmError> {
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

pub(crate) fn parse_scope(row: &sqlx::sqlite::SqliteRow) -> Result<Scope, CmError> {
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

pub(crate) fn parse_relation(row: &sqlx::sqlite::SqliteRow) -> Result<EntryRelation, CmError> {
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

pub(crate) fn parse_mutation(row: &sqlx::sqlite::SqliteRow) -> Result<MutationRecord, CmError> {
    let id_str: String = row.get("id");
    let entry_id_str: String = row.get("entry_id");
    let action_str: String = row.get("action");
    let source_str: String = row.get("source");
    let timestamp_str: String = row.get("timestamp");
    let before_str: Option<String> = row.get("before_snapshot");
    let after_str: Option<String> = row.get("after_snapshot");

    Ok(MutationRecord {
        id: Uuid::parse_str(&id_str)
            .map_err(|e| CmError::Internal(format!("invalid mutation id: {e}")))?,
        entry_id: Uuid::parse_str(&entry_id_str)
            .map_err(|e| CmError::Internal(format!("invalid entry_id: {e}")))?,
        action: action_str.parse()?,
        source: source_str.parse()?,
        timestamp: parse_datetime(&timestamp_str)?,
        before_snapshot: before_str.map(|s| serde_json::from_str(&s)).transpose()?,
        after_snapshot: after_str.map(|s| serde_json::from_str(&s)).transpose()?,
    })
}

pub(crate) fn parse_datetime(s: &str) -> Result<DateTime<Utc>, CmError> {
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

pub(crate) fn map_db_err(e: sqlx::Error) -> CmError {
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
