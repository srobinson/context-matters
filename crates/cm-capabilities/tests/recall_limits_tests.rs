//! Capability level tests for recall token budget and result limits.

mod common;

use cm_capabilities::recall::{RecallRequest, recall};
use cm_core::EntryKind;
use common::{create_global, seed_entry, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn token_budget_truncates_results() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..10 {
        seed_entry(
            &store,
            &format!("Entry {i}"),
            &format!("Body content for entry number {i} with padding text to consume tokens."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            max_tokens: Some(50),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(result.entries.len() < 10);
    assert!(result.token_estimate > 0);
    assert!(result.token_estimate <= 50 + 100);
}

#[tokio::test(flavor = "multi_thread")]
async fn token_budget_always_includes_first_entry() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let long_body = "x".repeat(1000);
    seed_entry(&store, "Large entry", &long_body, EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            max_tokens: Some(1),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.title, "Large entry");
    assert!(result.token_estimate > 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn token_budget_none_returns_all() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..5 {
        seed_entry(
            &store,
            &format!("Entry {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            max_tokens: None,
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn limit_caps_results_after_post_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..10 {
        seed_entry(
            &store,
            &format!("Fact {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            limit: 3,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 3);
}
