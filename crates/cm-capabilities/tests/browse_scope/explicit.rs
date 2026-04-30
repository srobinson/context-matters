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

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_selector_supports_subtree_set_and_all() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project decision",
        EntryKind::Decision,
        "global/project:helioy",
    )
    .await;
    seed_scoped(
        &store,
        "CM repo lesson",
        EntryKind::Lesson,
        "global/project:helioy/repo:cm",
    )
    .await;
    seed_scoped(
        &store,
        "AM repo fact",
        EntryKind::Fact,
        "global/project:attention-matters/repo:am",
    )
    .await;

    let helioy = ScopePath::parse("global/project:helioy").unwrap();
    let subtree = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::Subtree(helioy.clone())),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        sorted_titles(&subtree),
        ["CM repo lesson", "Project decision"]
    );
    assert!(subtree.resolution.is_none());

    let set = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::Set(vec![
                ScopePath::parse("global").unwrap(),
                ScopePath::parse("global/project:attention-matters/repo:am").unwrap(),
            ])),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(sorted_titles(&set), ["AM repo fact", "Global fact"]);
    assert!(set.resolution.is_none());

    let all = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::All),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        sorted_titles(&all),
        [
            "AM repo fact",
            "CM repo lesson",
            "Global fact",
            "Project decision"
        ]
    );
    assert!(all.resolution.is_none());
}

fn sorted_titles(result: &cm_capabilities::browse::BrowseResult) -> Vec<&str> {
    let mut titles = result
        .entries
        .iter()
        .map(|entry| entry.title.as_str())
        .collect::<Vec<_>>();
    titles.sort_unstable();
    titles
}

#[test]
fn browse_selector_rejects_removed_auto_value() {
    let err = ScopeSelector::parse("auto").unwrap_err();

    assert!(
        err.to_string().contains("instead of scope='auto'"),
        "unexpected error: {err}",
    );
}
