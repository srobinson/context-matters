//! Capability level tests for RecallRow score semantics.

mod common;

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{EntryKind, ScopePath};
use common::{create_global, seed_entry, seed_entry_with_scope, seed_entry_with_tags, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn recall_row_score_is_some_on_search_routing() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "sqlx migration guide",
        "sqlx migration sqlx migration sqlx",
        EntryKind::Reference,
    )
    .await;
    seed_entry(
        &store,
        "Unrelated title",
        "Passing mention of sqlx.",
        EntryKind::Fact,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert!(result.entries.len() >= 2);
    for row in &result.entries {
        let score = row
            .score
            .expect("Search routing must populate RecallRow.score");
        assert!(score.is_finite());
        assert!(score < 0.0, "bm25 rank should be negative, got {score}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_row_score_is_none_on_default_scope_resolution() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact one", "Body one.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert!(!result.entries.is_empty());
    for row in &result.entries {
        assert!(row.score.is_none(), "ScopeResolve must leave score None");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_row_score_is_none_on_tag_scope_walk() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Tagged fact",
        "Body with session tag.",
        EntryKind::Fact,
        vec!["session-log".to_owned()],
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            tags: vec!["session-log".to_owned()],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::TagScopeWalk);
    assert!(!result.entries.is_empty());
    for row in &result.entries {
        assert!(row.score.is_none(), "TagScopeWalk must leave score None");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_row_score_is_none_on_scope_resolve() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Global preference",
        "Use rfc3339.",
        EntryKind::Preference,
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Project fact",
        "Helioy uses monorepo.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert!(!result.entries.is_empty());
    for row in &result.entries {
        assert!(row.score.is_none(), "ScopeResolve must leave score None");
    }
}
