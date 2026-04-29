use super::support::{seed_scoped, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_capabilities::scope::ScopeSelector;
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
async fn browse_scope_selector_exact_path_filters_exactly() {
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
            scope: Some(ScopeSelector::Path(project_scope)),
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

#[test]
fn browse_selector_rejects_removed_auto_value() {
    let err = ScopeSelector::parse("auto").unwrap_err();

    assert!(
        err.to_string().contains("instead of scope='auto'"),
        "unexpected error: {err}",
    );
}
