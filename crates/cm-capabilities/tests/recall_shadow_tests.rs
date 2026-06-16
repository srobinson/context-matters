mod common;

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{Confidence, EntryKind, EntryMeta, RecallRankingMode, ScopePath};
use serde_json::Value;
use sqlx::Row;

use common::{CANONICAL_CONTEXT_REPO_SCOPE, seed_entry_with_meta, test_store_with_ranking_mode};

#[tokio::test(flavor = "current_thread")]
async fn shadow_serves_legacy_order_and_logs_one_row() {
    let (store, _db_dir) = test_store_with_ranking_mode(RecallRankingMode::Shadow).await;

    seed_entry_with_meta(
        &store,
        "Global high priority",
        "rankneedle",
        EntryKind::Fact,
        "global",
        EntryMeta {
            confidence: Some(Confidence::Medium),
            priority: Some(10),
            ..Default::default()
        },
    )
    .await;
    seed_entry_with_meta(
        &store,
        "Repo default priority",
        "rankneedle",
        EntryKind::Fact,
        CANONICAL_CONTEXT_REPO_SCOPE,
        EntryMeta {
            confidence: Some(Confidence::Medium),
            priority: Some(0),
            ..Default::default()
        },
    )
    .await;

    let result = recall(&store, query_request("rankneedle", 2))
        .await
        .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(
        titles(&result),
        vec!["Repo default priority", "Global high priority"]
    );
    assert_eq!(result.candidates_before_filter, 2);
    assert_eq!(result.fetch_limit_used, 2);
    assert_eq!(shadow_count(&store).await, 1);

    let row = shadow_row(&store, "search").await;
    assert_eq!(row.get::<i64, _>("top1_changed"), 1);
    assert_eq!(row.get::<f64, _>("topk_overlap"), 1.0);
    assert_eq!(row.get::<f64, _>("footrule"), 1.0);
    assert_eq!(row.get::<f64, _>("mean_abs_position_delta"), 1.0);
    assert_eq!(row.get::<String, _>("routing"), "search");
    assert_eq!(row.get::<String, _>("tier"), "exact");
    assert!(row.get::<String, _>("query_hash").len() >= 32);
    assert_eq!(row.get::<i64, _>("query_len"), 10);
    assert_eq!(json_len(&row, "position_deltas"), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn shadow_marks_window_truncated_for_priority_promotion_beyond_legacy_window() {
    let (store, _db_dir) = test_store_with_ranking_mode(RecallRankingMode::Shadow).await;

    seed_entry_with_meta(
        &store,
        "Global feedback",
        "important",
        EntryKind::Feedback,
        "global",
        EntryMeta {
            confidence: Some(Confidence::High),
            priority: Some(100),
            ..Default::default()
        },
    )
    .await;
    for index in 1..=3 {
        seed_entry_with_meta(
            &store,
            &format!("Repo observation {index}"),
            &format!("ordinary {index}"),
            EntryKind::Observation,
            CANONICAL_CONTEXT_REPO_SCOPE,
            EntryMeta::default(),
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            scope: Some(ScopeSelector::Path(
                ScopePath::parse(CANONICAL_CONTEXT_REPO_SCOPE).unwrap(),
            )),
            limit: 2,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert_eq!(result.entries.len(), 2);
    assert!(
        titles(&result)
            .iter()
            .all(|title| title.starts_with("Repo"))
    );

    let row = shadow_row(&store, "scope_resolve").await;
    assert_eq!(row.get::<Option<String>, _>("query_hash"), None);
    assert_eq!(row.get::<Option<i64>, _>("query_len"), None);
    assert_eq!(row.get::<Option<String>, _>("tier"), None);
    assert_eq!(row.get::<i64, _>("candidate_count"), 4);
    assert_eq!(row.get::<i64, _>("window_truncated"), 1);
    assert_eq!(row.get::<i64, _>("top1_changed"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn shadow_log_error_does_not_fail_recall() {
    let (store, _db_dir) = test_store_with_ranking_mode(RecallRankingMode::Shadow).await;
    sqlx::query("DROP TABLE recall_shadow")
        .execute(store.write_pool())
        .await
        .unwrap();

    seed_entry_with_meta(
        &store,
        "Recall survives",
        "rankneedle",
        EntryKind::Fact,
        "global",
        EntryMeta::default(),
    )
    .await;

    let result = recall(&store, query_request("rankneedle", 10)).await;

    assert!(result.is_ok());
    assert_eq!(titles(&result.unwrap()), vec!["Recall survives"]);
}

fn query_request(query: &str, limit: u32) -> RecallRequest {
    RecallRequest {
        query: Some(query.to_owned()),
        scope: Some(ScopeSelector::Path(
            ScopePath::parse(CANONICAL_CONTEXT_REPO_SCOPE).unwrap(),
        )),
        limit,
        ..Default::default()
    }
}

async fn shadow_count(store: &cm_store::CmStore) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM recall_shadow")
        .fetch_one(store.write_pool())
        .await
        .unwrap()
}

async fn shadow_row(store: &cm_store::CmStore, routing: &str) -> sqlx::sqlite::SqliteRow {
    sqlx::query("SELECT * FROM recall_shadow WHERE routing = ?")
        .bind(routing)
        .fetch_one(store.write_pool())
        .await
        .unwrap()
}

fn json_len(row: &sqlx::sqlite::SqliteRow, column: &str) -> usize {
    let json: String = row.get(column);
    let value: Value = serde_json::from_str(&json).unwrap();
    value.as_array().unwrap().len()
}

fn titles(result: &cm_capabilities::recall::RecallResult) -> Vec<&str> {
    result
        .entries
        .iter()
        .map(|row| row.entry.title.as_str())
        .collect()
}
