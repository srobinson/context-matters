use super::support::{seed_scoped_with_details, test_store, wctx};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_core::{BrowseSort, ContextStore, EntryKind, EntryMeta, NewEntry, ScopePath};

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_preserves_other_filters_and_pagination() {
    let (store, _dir) = test_store().await;
    let repo_scope = "global/project:helioy/repo:context-matters";

    let original = seed_scoped_with_details(
        &store,
        "Scoped 00",
        EntryKind::Fact,
        repo_scope,
        "agent:auto",
        &["keep"],
    )
    .await;
    store
        .supersede_entry(
            original.id,
            NewEntry {
                scope_path: ScopePath::parse(repo_scope).unwrap(),
                kind: EntryKind::Fact,
                title: "Scoped 04".to_owned(),
                body: "Replacement body.".to_owned(),
                created_by: "agent:auto".to_owned(),
                meta: Some(EntryMeta {
                    tags: vec!["keep".to_owned()],
                    ..Default::default()
                }),
            },
            &wctx(),
        )
        .await
        .unwrap();

    for title in ["Scoped 01", "Scoped 02", "Scoped 03"] {
        seed_scoped_with_details(
            &store,
            title,
            EntryKind::Fact,
            repo_scope,
            "agent:auto",
            &["keep"],
        )
        .await;
    }

    seed_scoped_with_details(
        &store,
        "Wrong kind",
        EntryKind::Decision,
        repo_scope,
        "agent:auto",
        &["keep"],
    )
    .await;
    seed_scoped_with_details(
        &store,
        "Wrong tag",
        EntryKind::Fact,
        repo_scope,
        "agent:auto",
        &["drop"],
    )
    .await;
    seed_scoped_with_details(
        &store,
        "Wrong creator",
        EntryKind::Fact,
        repo_scope,
        "agent:other",
        &["keep"],
    )
    .await;
    seed_scoped_with_details(
        &store,
        "Wrong scope",
        EntryKind::Fact,
        "global",
        "agent:auto",
        &["keep"],
    )
    .await;

    let page1 = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/context-matters".into()),
            kind: Some(EntryKind::Fact),
            tag: Some("keep".to_owned()),
            created_by: Some("agent:auto".to_owned()),
            include_superseded: true,
            sort: BrowseSort::TitleAsc,
            limit: Some(2),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page1.total, 5);
    assert_eq!(page1.entries.len(), 2);
    assert_eq!(page1.entries[0].title, "Scoped 00");
    assert_eq!(page1.entries[1].title, "Scoped 01");
    assert!(page1.has_more);
    assert_eq!(
        page1.resolution.as_ref().unwrap().resolved_scope,
        ScopePath::parse(repo_scope).unwrap()
    );

    let page2 = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/context-matters".into()),
            kind: Some(EntryKind::Fact),
            tag: Some("keep".to_owned()),
            created_by: Some("agent:auto".to_owned()),
            include_superseded: true,
            sort: BrowseSort::TitleAsc,
            limit: Some(2),
            cursor: page1.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page2.total, 5);
    assert_eq!(page2.entries.len(), 2);
    assert_eq!(page2.entries[0].title, "Scoped 02");
    assert_eq!(page2.entries[1].title, "Scoped 03");
}
