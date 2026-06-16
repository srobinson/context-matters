mod common;

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{Confidence, EntryKind, EntryMeta, RecallRankingMode, ScopePath};
use cm_store::CmStore;

use common::{
    CANONICAL_CONTEXT_REPO_SCOPE, seed_entry_with_meta, test_store, test_store_with_ranking_mode,
};

#[tokio::test(flavor = "current_thread")]
async fn default_legacy_order_stays_scope_depth_first() {
    let (store, _db_dir) = test_store().await;

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

    let result = recall(&store, ranking_request("rankneedle")).await.unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(
        titles(&result),
        vec!["Repo default priority", "Global high priority"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn live_orders_by_priority_before_scope_depth() {
    let (store, _db_dir) = test_store_with_ranking_mode(RecallRankingMode::Live).await;

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

    let result = recall(&store, ranking_request("rankneedle")).await.unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(
        titles(&result),
        vec!["Global high priority", "Repo default priority"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn live_orders_by_kind_tier_and_confidence() {
    let (store, _db_dir) = test_store_with_ranking_mode(RecallRankingMode::Live).await;

    seed_global_rank_entry(&store, "Fact high", EntryKind::Fact, Some(Confidence::High)).await;
    seed_global_rank_entry(
        &store,
        "Decision medium",
        EntryKind::Decision,
        Some(Confidence::Medium),
    )
    .await;
    seed_global_rank_entry(
        &store,
        "Decision high",
        EntryKind::Decision,
        Some(Confidence::High),
    )
    .await;
    seed_global_rank_entry(
        &store,
        "Feedback low",
        EntryKind::Feedback,
        Some(Confidence::Low),
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            scope: Some(ScopeSelector::Path(ScopePath::global())),
            limit: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert_eq!(
        titles(&result),
        vec![
            "Feedback low",
            "Decision high",
            "Decision medium",
            "Fact high"
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn live_uses_bm25_as_tiebreak_before_recency_and_id() {
    let (store, _db_dir) = test_store_with_ranking_mode(RecallRankingMode::Live).await;

    seed_entry_with_meta(
        &store,
        "Deep match",
        "rankneedle rankneedle rankneedle",
        EntryKind::Fact,
        CANONICAL_CONTEXT_REPO_SCOPE,
        EntryMeta {
            confidence: Some(Confidence::Medium),
            priority: Some(0),
            ..Default::default()
        },
    )
    .await;
    seed_entry_with_meta(
        &store,
        "Weak match",
        "rankneedle once",
        EntryKind::Fact,
        CANONICAL_CONTEXT_REPO_SCOPE,
        EntryMeta {
            confidence: Some(Confidence::Medium),
            priority: Some(0),
            ..Default::default()
        },
    )
    .await;

    let result = recall(&store, ranking_request("rankneedle")).await.unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(titles(&result), vec!["Deep match", "Weak match"]);
    let first_score = result.entries[0].score.unwrap();
    let second_score = result.entries[1].score.unwrap();
    assert!(
        first_score < second_score,
        "lower BM25 score should rank first: {first_score} vs {second_score}"
    );
}

fn ranking_request(query: &str) -> RecallRequest {
    RecallRequest {
        query: Some(query.to_owned()),
        scope: Some(ScopeSelector::Path(
            ScopePath::parse(CANONICAL_CONTEXT_REPO_SCOPE).unwrap(),
        )),
        limit: 10,
        ..Default::default()
    }
}

async fn seed_global_rank_entry(
    store: &CmStore,
    title: &str,
    kind: EntryKind,
    confidence: Option<Confidence>,
) {
    let body = format!("{title} kind rank body");
    seed_entry_with_meta(
        store,
        title,
        &body,
        kind,
        "global",
        EntryMeta {
            confidence,
            priority: Some(0),
            ..Default::default()
        },
    )
    .await;
}

fn titles(result: &cm_capabilities::recall::RecallResult) -> Vec<&str> {
    result
        .entries
        .iter()
        .map(|row| row.entry.title.as_str())
        .collect()
}
