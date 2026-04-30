//! Capability level tests for recall filtering and fetch limit compensation.

mod common;

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{CmError, EntryKind, ScopePath};
use common::{create_global, seed_entry, seed_entry_with_tags, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn default_scope_resolution_passes_single_kind_to_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Fact body.", EntryKind::Fact).await;
    seed_entry(&store, "A decision", "Decision body.", EntryKind::Decision).await;

    let result = recall(
        &store,
        RecallRequest {
            kinds: vec![EntryKind::Fact],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.kind, EntryKind::Fact);
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_rejects_non_singular_scope_selectors() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let project = ScopePath::parse("global/project:helioy").unwrap();

    for scope in [
        ScopeSelector::Subtree(project.clone()),
        ScopeSelector::Set(vec![project]),
        ScopeSelector::All,
    ] {
        let err = recall(
            &store,
            RecallRequest {
                scope: Some(scope),
                limit: 10,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(
            err,
            CmError::InvalidOperationInput {
                op: "cx_recall",
                ..
            }
        ));
        assert!(err.to_string().contains("cx_search"));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn search_post_filters_by_kinds() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Rust fact about sqlx",
        "Use sqlx for queries.",
        EntryKind::Fact,
    )
    .await;
    seed_entry(
        &store,
        "Rust decision about sqlx",
        "We decided to use sqlx.",
        EntryKind::Decision,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            kinds: vec![EntryKind::Fact],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.kind, EntryKind::Fact);
    assert!(result.candidates_before_filter >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_post_filters_by_tags() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Tagged sqlx note",
        "Use sqlx for database queries.",
        EntryKind::Fact,
        vec!["database".to_owned()],
    )
    .await;
    seed_entry(
        &store,
        "Untagged sqlx note",
        "Also about sqlx usage.",
        EntryKind::Fact,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            tags: vec!["database".to_owned()],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.title, "Tagged sqlx note");
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_limit_compensates_when_post_filters_active() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..6 {
        seed_entry(
            &store,
            &format!("Fact {i}"),
            &format!("Searchable content about topic alpha {i}."),
            EntryKind::Fact,
        )
        .await;
    }
    for i in 0..4 {
        seed_entry(
            &store,
            &format!("Decision {i}"),
            &format!("Searchable content about topic alpha {i}."),
            EntryKind::Decision,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            query: Some("alpha".to_owned()),
            kinds: vec![EntryKind::Fact],
            limit: 5,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.fetch_limit_used, 15);
    for row in &result.entries {
        assert_eq!(row.entry.kind, EntryKind::Fact);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_limit_unchanged_without_post_filters() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Simple fact", "Basic content.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.fetch_limit_used, 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn default_scope_resolution_filters_multiple_kinds() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Body A.", EntryKind::Fact).await;
    seed_entry(&store, "A decision", "Body B.", EntryKind::Decision).await;
    seed_entry(&store, "A lesson", "Body C.", EntryKind::Lesson).await;
    seed_entry(&store, "A pattern", "Body D.", EntryKind::Pattern).await;

    let result = recall(
        &store,
        RecallRequest {
            kinds: vec![EntryKind::Fact, EntryKind::Decision],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert_eq!(result.entries.len(), 2);
    for row in &result.entries {
        assert!(
            row.entry.kind == EntryKind::Fact || row.entry.kind == EntryKind::Decision,
            "unexpected kind: {:?}",
            row.entry.kind,
        );
    }
}
