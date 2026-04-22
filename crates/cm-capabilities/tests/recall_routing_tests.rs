//! Capability level tests for recall routing branch selection.

mod common;

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_core::{EntryKind, ScopePath};
use common::{
    create_global, seed_entry, seed_entry_with_scope, seed_entry_with_tags,
    seed_scoped_tagged_entry, test_store,
};

#[tokio::test(flavor = "multi_thread")]
async fn routing_search_when_query_provided() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "SQLx migration guide",
        "Run sqlx migrate to apply.",
        EntryKind::Reference,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx migrate".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert!(!result.entries.is_empty());
    assert_eq!(result.entries[0].entry.title, "SQLx migration guide");
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_search_with_scope_filters_to_ancestors() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Global sqlx note",
        "Use sqlx for DB.",
        EntryKind::Fact,
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Project sqlx note",
        "Use sqlx migrations in helioy.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            scope: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert!(result.entries.len() >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_tag_scope_walk_when_tags_no_query() {
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
    seed_entry(&store, "Untagged fact", "No tags here.", EntryKind::Fact).await;

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
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.title, "Tagged fact");
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_tag_scope_walk_with_scope_walks_ancestors() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Global tagged",
        "At global scope.",
        EntryKind::Fact,
        vec!["infra".to_owned()],
    )
    .await;
    seed_scoped_tagged_entry(
        &store,
        "Project tagged",
        "At project scope.",
        EntryKind::Fact,
        "global/project:helioy",
        vec!["infra".to_owned()],
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            tags: vec!["infra".to_owned()],
            scope: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::TagScopeWalk);
    assert_eq!(result.entries.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_scope_resolve_when_scope_no_query_no_tags() {
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
            scope: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert!(result.entries.len() >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_scope_resolve_when_no_query_no_scope_no_tags() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact one", "Body one.", EntryKind::Fact).await;
    seed_entry(&store, "Fact two", "Body two.", EntryKind::Decision).await;

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
    assert_eq!(result.entries.len(), 2);
}
