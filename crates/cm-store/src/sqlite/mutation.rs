//! Mutation record helpers.
//!
//! `entry_snapshot` and `insert_mutation` are used within transactions
//! by the entry write methods. Separated here to keep the write methods
//! focused on business logic.

use chrono::Utc;
use cm_core::{CmError, Entry, MutationAction, MutationSource};
use uuid::Uuid;

use super::parse::map_db_err;

/// Serialize an Entry to a JSON Value for mutation snapshots.
pub(crate) fn entry_snapshot(entry: &Entry) -> Result<serde_json::Value, CmError> {
    Ok(serde_json::to_value(entry)?)
}

/// Insert a mutation record within an existing transaction.
pub(crate) async fn insert_mutation(
    executor: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    entry_id: &str,
    action: MutationAction,
    source: MutationSource,
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> Result<(), CmError> {
    let id = Uuid::now_v7().to_string();
    let action_str = action.as_str();
    let source_str = source.as_str();
    let before_json = before.map(|v| v.to_string());
    let after_json = after.map(|v| v.to_string());
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO mutations (id, entry_id, action, source, timestamp, before_snapshot, after_snapshot) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(entry_id)
    .bind(action_str)
    .bind(source_str)
    .bind(&now)
    .bind(&before_json)
    .bind(&after_json)
    .execute(executor)
    .await
    .map_err(map_db_err)?;

    Ok(())
}
