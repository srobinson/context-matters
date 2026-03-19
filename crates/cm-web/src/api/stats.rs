//! Dashboard stats endpoint with supplementary queries.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use cm_core::ContextStore;
use serde::Serialize;
use sqlx::Row;

use crate::AppState;
use crate::api::error::ApiError;

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    // Base store stats (flattened)
    pub active_entries: u64,
    pub superseded_entries: u64,
    pub scopes: u64,
    pub relations: u64,
    pub entries_by_kind: HashMap<String, u64>,
    pub entries_by_scope: HashMap<String, u64>,
    pub entries_by_tag: Vec<cm_core::TagCount>,
    pub db_size_bytes: u64,

    // Supplementary: activity
    pub entries_today: u64,
    pub entries_this_week: u64,

    // Supplementary: agents
    pub active_agents: Vec<AgentActivity>,

    // Supplementary: scope tree
    pub scope_tree: Vec<ScopeNode>,

    // Supplementary: quality snapshot
    pub quality: QualitySnapshot,
}

#[derive(Debug, Serialize)]
pub struct AgentActivity {
    pub created_by: String,
    pub count: u64,
}

#[derive(Debug, Serialize)]
pub struct ScopeNode {
    pub path: String,
    pub kind: String,
    pub entry_count: u64,
}

#[derive(Debug, Serialize)]
pub struct QualitySnapshot {
    pub untagged_count: u64,
    pub stale_count: u64,
    pub global_scope_count: u64,
}

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DashboardStats>, ApiError> {
    let pool = state.store.read_pool();

    // Base stats from store trait
    let base = state.store.stats().await?;

    // Entries created today (UTC)
    let today_row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM entries \
         WHERE superseded_by IS NULL \
         AND date(created_at) = date('now')",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;
    let entries_today: i64 = today_row.get("cnt");

    // Entries created this week (last 7 days)
    let week_row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM entries \
         WHERE superseded_by IS NULL \
         AND created_at >= datetime('now', '-7 days')",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;
    let entries_this_week: i64 = week_row.get("cnt");

    // Active agents: distinct created_by with counts (written in last 7 days)
    let agent_rows = sqlx::query(
        "SELECT created_by, COUNT(*) as cnt FROM entries \
         WHERE superseded_by IS NULL \
         AND created_at >= datetime('now', '-7 days') \
         AND created_by IS NOT NULL AND created_by != '' \
         GROUP BY created_by ORDER BY cnt DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;

    let active_agents: Vec<AgentActivity> = agent_rows
        .iter()
        .map(|row| AgentActivity {
            created_by: row.get("created_by"),
            count: row.get::<i64, _>("cnt") as u64,
        })
        .collect();

    // Scope tree: all scopes with entry counts
    let scope_rows = sqlx::query(
        "SELECT s.path, s.kind, \
         (SELECT COUNT(*) FROM entries e WHERE e.scope_path = s.path AND e.superseded_by IS NULL) as entry_count \
         FROM scopes s ORDER BY s.path ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;

    let scope_tree: Vec<ScopeNode> = scope_rows
        .iter()
        .map(|row| ScopeNode {
            path: row.get("path"),
            kind: row.get("kind"),
            entry_count: row.get::<i64, _>("entry_count") as u64,
        })
        .collect();

    // Quality snapshot
    // Untagged: active entries with no tags (meta IS NULL or tags is empty)
    let untagged_row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM entries \
         WHERE superseded_by IS NULL \
         AND (meta IS NULL OR json_array_length(json_extract(meta, '$.tags')) = 0 \
              OR json_extract(meta, '$.tags') IS NULL)",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;
    let untagged_count: i64 = untagged_row.get("cnt");

    // Stale: active entries not updated in 30+ days
    let stale_row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM entries \
         WHERE superseded_by IS NULL \
         AND updated_at < datetime('now', '-30 days')",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;
    let stale_count: i64 = stale_row.get("cnt");

    // Global scope: active entries at exactly 'global' scope
    let global_row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM entries \
         WHERE superseded_by IS NULL AND scope_path = 'global'",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError(cm_core::CmError::Database(e.to_string())))?;
    let global_scope_count: i64 = global_row.get("cnt");

    Ok(Json(DashboardStats {
        active_entries: base.active_entries,
        superseded_entries: base.superseded_entries,
        scopes: base.scopes,
        relations: base.relations,
        entries_by_kind: base.entries_by_kind,
        entries_by_scope: base.entries_by_scope,
        entries_by_tag: base.entries_by_tag,
        db_size_bytes: base.db_size_bytes,
        entries_today: entries_today as u64,
        entries_this_week: entries_this_week as u64,
        active_agents,
        scope_tree,
        quality: QualitySnapshot {
            untagged_count: untagged_count as u64,
            stale_count: stale_count as u64,
            global_scope_count: global_scope_count as u64,
        },
    }))
}
