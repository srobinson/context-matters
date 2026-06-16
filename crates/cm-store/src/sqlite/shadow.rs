//! Recall shadow canary persistence.

use cm_core::{
    CmError, RecallShadowListFilter, RecallShadowRecord, RecallShadowRow, RecallShadowSummary,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite};
use uuid::Uuid;

use super::CmStore;
use super::parse::{map_db_err, parse_datetime};

impl CmStore {
    pub(crate) async fn do_log_recall_shadow(
        &self,
        record: RecallShadowRecord,
    ) -> Result<(), CmError> {
        let id = Uuid::now_v7().to_string();
        let position_deltas = serde_json::to_string(&record.position_deltas)?;
        let old_ids = serde_json::to_string(&record.old_ids)?;
        let new_ids = serde_json::to_string(&record.new_ids)?;

        sqlx::query(
            "INSERT INTO recall_shadow (\
             id, scope_path, query_hash, query_len, routing, tier, k, candidate_count, \
             top1_changed, topk_overlap, footrule, mean_abs_position_delta, position_deltas, \
             old_ids, new_ids, window_truncated, ranking_version, duration_ms\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(record.scope_path)
        .bind(record.query_hash)
        .bind(record.query_len.map(i64::from))
        .bind(record.routing)
        .bind(record.tier)
        .bind(i64::from(record.k))
        .bind(i64::from(record.candidate_count))
        .bind(record.top1_changed)
        .bind(record.topk_overlap)
        .bind(record.footrule)
        .bind(record.mean_abs_position_delta)
        .bind(position_deltas)
        .bind(old_ids)
        .bind(new_ids)
        .bind(record.window_truncated)
        .bind(record.ranking_version)
        .bind(i64::from(record.duration_ms))
        .execute(&self.write_pool)
        .await
        .map_err(map_db_err)?;

        Ok(())
    }

    pub async fn list_recall_shadow(
        &self,
        filter: &RecallShadowListFilter,
    ) -> Result<Vec<RecallShadowRow>, CmError> {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT id, ts, scope_path, query_hash, query_len, routing, tier, k, \
             candidate_count, top1_changed, topk_overlap, footrule, mean_abs_position_delta, \
             position_deltas, old_ids, new_ids, window_truncated, ranking_version, duration_ms \
             FROM recall_shadow WHERE 1=1",
        );

        push_recall_shadow_filters(&mut query, filter);
        query.push(" ORDER BY ts DESC, id DESC LIMIT ");
        query.push_bind(i64::from(filter.limit.clamp(1, 200)));

        let rows = query
            .build()
            .fetch_all(&self.read_pool)
            .await
            .map_err(map_db_err)?;

        rows.iter().map(parse_recall_shadow).collect()
    }

    pub async fn recall_shadow_summary(
        &self,
        filter: &RecallShadowListFilter,
    ) -> Result<RecallShadowSummary, CmError> {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) AS total, \
             COALESCE(1.0 * SUM(CASE WHEN top1_changed != 0 THEN 1 ELSE 0 END) / \
             NULLIF(COUNT(*), 0), 0.0) AS divergence_rate, \
             COALESCE(AVG(topk_overlap), 0.0) AS avg_topk_overlap, \
             COALESCE(AVG(footrule), 0.0) AS avg_footrule \
             FROM recall_shadow WHERE 1=1",
        );

        push_recall_shadow_filters(&mut query, filter);

        let row = query
            .build()
            .fetch_one(&self.read_pool)
            .await
            .map_err(map_db_err)?;

        Ok(RecallShadowSummary {
            total: u64_from_i64(row.get("total"))?,
            divergence_rate: row.get("divergence_rate"),
            avg_topk_overlap: row.get("avg_topk_overlap"),
            avg_footrule: row.get("avg_footrule"),
        })
    }
}

fn push_recall_shadow_filters<'args>(
    query: &mut QueryBuilder<'args, Sqlite>,
    filter: &'args RecallShadowListFilter,
) {
    if let Some(routing) = &filter.routing {
        query.push(" AND routing = ");
        query.push_bind(routing);
    }
    if let Some(scope_path) = &filter.scope_path {
        query.push(" AND scope_path = ");
        query.push_bind(scope_path);
    }
    if let Some(top1_changed) = filter.top1_changed {
        query.push(" AND top1_changed = ");
        query.push_bind(top1_changed);
    }
}

fn parse_recall_shadow(row: &SqliteRow) -> Result<RecallShadowRow, CmError> {
    let id_str: String = row.get("id");
    let ts_str: String = row.get("ts");
    let position_deltas: String = row.get("position_deltas");
    let old_ids: String = row.get("old_ids");
    let new_ids: String = row.get("new_ids");

    Ok(RecallShadowRow {
        id: Uuid::parse_str(&id_str)
            .map_err(|e| CmError::Internal(format!("invalid recall_shadow id: {e}")))?,
        ts: parse_datetime(&ts_str)?,
        scope_path: row.get("scope_path"),
        query_hash: row.get("query_hash"),
        query_len: optional_u32(row, "query_len")?,
        routing: row.get("routing"),
        tier: row.get("tier"),
        k: required_u32(row, "k")?,
        candidate_count: required_u32(row, "candidate_count")?,
        top1_changed: row.get::<i64, _>("top1_changed") != 0,
        topk_overlap: row.get("topk_overlap"),
        footrule: row.get("footrule"),
        mean_abs_position_delta: row.get("mean_abs_position_delta"),
        position_deltas: serde_json::from_str(&position_deltas)?,
        old_ids: serde_json::from_str(&old_ids)?,
        new_ids: serde_json::from_str(&new_ids)?,
        window_truncated: row.get::<i64, _>("window_truncated") != 0,
        ranking_version: row.get("ranking_version"),
        duration_ms: required_u32(row, "duration_ms")?,
    })
}

fn optional_u32(row: &SqliteRow, column: &str) -> Result<Option<u32>, CmError> {
    row.get::<Option<i64>, _>(column)
        .map(u32_from_i64)
        .transpose()
}

fn required_u32(row: &SqliteRow, column: &str) -> Result<u32, CmError> {
    u32_from_i64(row.get(column))
}

fn u32_from_i64(value: i64) -> Result<u32, CmError> {
    u32::try_from(value).map_err(|e| CmError::Internal(format!("invalid u32 value {value}: {e}")))
}

fn u64_from_i64(value: i64) -> Result<u64, CmError> {
    u64::try_from(value).map_err(|e| CmError::Internal(format!("invalid u64 value {value}: {e}")))
}
