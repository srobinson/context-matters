//! Recall shadow canary persistence.

use cm_core::{CmError, RecallShadowRecord};
use uuid::Uuid;

use super::CmStore;
use super::parse::map_db_err;

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
}
