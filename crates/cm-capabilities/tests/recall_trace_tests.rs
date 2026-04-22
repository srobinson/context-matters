//! Capability level tests for recall scope chain and trace metadata.

mod common;

use cm_capabilities::constants::MAX_LIMIT;
use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_core::{EntryKind, ScopePath};
use common::{create_global, seed_entry, seed_entry_with_scope, seed_entry_with_tags, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn scope_chain_extracted_from_scope_path() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact", "Body.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            scope: Some(ScopePath::parse("global/project:helioy/repo:cm").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        result.scope_chain,
        vec![
            "global/project:helioy/repo:cm",
            "global/project:helioy",
            "global"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn scope_chain_uses_default_scope_when_no_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact", "Body.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.scope_chain, vec!["global"]);
    assert_eq!(result.scope_hits, vec![("global".to_owned(), 1)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn result_includes_trace_metadata() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Body.", EntryKind::Fact).await;
    seed_entry(&store, "A decision", "Body.", EntryKind::Decision).await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("body".to_owned()),
            kinds: vec![EntryKind::Fact],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(result.candidates_before_filter >= 2);
    assert_eq!(result.fetch_limit_used, 60);
    assert_eq!(result.routing, RecallRouting::Search);
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_scope_walk_trace_metadata_reports_max_limit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Tagged A",
        "Body A.",
        EntryKind::Fact,
        vec!["infra".to_owned()],
    )
    .await;
    seed_entry_with_tags(
        &store,
        "Tagged B",
        "Body B.",
        EntryKind::Fact,
        vec!["infra".to_owned()],
    )
    .await;
    seed_entry(&store, "Plain", "No tags.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            tags: vec!["infra".to_owned()],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::TagScopeWalk);
    assert_eq!(result.fetch_limit_used, MAX_LIMIT);
    assert!(result.candidates_before_filter >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn scope_resolve_trace_metadata_populated() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Global fact", "Body.", EntryKind::Fact).await;
    seed_entry_with_scope(
        &store,
        "Project fact",
        "Body.",
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
    assert_eq!(result.fetch_limit_used, 20);
    assert!(result.candidates_before_filter >= 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn default_scope_resolution_trace_metadata_populated() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact one", "Body one.", EntryKind::Fact).await;
    seed_entry(&store, "Fact two", "Body two.", EntryKind::Fact).await;

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
    assert_eq!(result.fetch_limit_used, 20);
    assert!(result.candidates_before_filter >= 2);
}
