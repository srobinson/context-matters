use super::support::{
    create_global, seed_entry, seed_entry_with_scope, seed_entry_with_tags, seed_with_creator,
    test_store,
};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{EntryKind, ScopePath};

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_exact_scope_selector() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Global fact", "At global.", EntryKind::Fact).await;
    seed_entry_with_scope(
        &store,
        "Project fact",
        "Body for Project fact.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Project fact");
    assert!(result.resolution.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_kind() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Body.", EntryKind::Fact).await;
    seed_entry(&store, "A decision", "Body.", EntryKind::Decision).await;
    seed_entry(&store, "A lesson", "Body.", EntryKind::Lesson).await;

    let result = browse(
        &store,
        BrowseRequest {
            kind: Some(EntryKind::Decision),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].kind, EntryKind::Decision);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_tag() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Tagged entry",
        "Body for Tagged entry.",
        EntryKind::Fact,
        vec!["infra".to_owned()],
    )
    .await;
    seed_entry(&store, "Untagged entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            tag: Some("infra".to_owned()),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Tagged entry");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_created_by() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_with_creator(&store, "Agent entry", EntryKind::Fact, "agent:nancy").await;
    seed_with_creator(&store, "MCP entry", EntryKind::Fact, "mcp:claude").await;

    let result = browse(
        &store,
        BrowseRequest {
            created_by: Some("agent:nancy".to_owned()),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Agent entry");
}
