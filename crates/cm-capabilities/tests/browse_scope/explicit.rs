use super::support::{seed_scoped, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_core::{EntryKind, ScopePath};

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_explicit_path_filters_exactly() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("global/project:helioy".to_owned()),
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
async fn browse_matching_scope_and_scope_path_filter_exactly() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;
    let project_scope = ScopePath::parse("global/project:helioy").unwrap();

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some(project_scope.as_str().to_owned()),
            scope_path: Some(project_scope),
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
async fn browse_scope_and_scope_path_must_not_conflict() {
    let (store, _dir) = test_store().await;

    let err = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            scope_path: Some(ScopePath::parse("global").unwrap()),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("cannot be combined with scope_path"),
        "unexpected error: {err}",
    );
}
