//! Scope-scoped browse and context resolution tests.

mod common;

use cm_core::{EntryFilter, EntryKind, NewScope};
use common::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c13_query_by_scope_returns_exact_scope_only() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:alpha").unwrap();
    store
        .create_scope(
            NewScope {
                path: project_path.clone(),
                label: "Alpha".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Global entry", "At global"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:alpha",
                EntryKind::Fact,
                "Project entry",
                "At project",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let result = store
        .browse(EntryFilter {
            scope_path: Some(project_path),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Project entry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c14_resolve_context_returns_ancestors_most_specific_first() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:helioy").unwrap();
    store
        .create_scope(
            NewScope {
                path: project_path.clone(),
                label: "Helioy".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let repo_path = ScopePath::parse("global/project:helioy/repo:nancyr").unwrap();
    store
        .create_scope(
            NewScope {
                path: repo_path.clone(),
                label: "nancyr".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Global fact", "Global body"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:helioy",
                EntryKind::Decision,
                "Project decision",
                "Project body",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:helioy/repo:nancyr",
                EntryKind::Lesson,
                "Repo lesson",
                "Repo body",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let entries = store.resolve_context(&repo_path, &[], 100).await.unwrap();

    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries[0].scope_path.as_str(),
        "global/project:helioy/repo:nancyr"
    );
    assert_eq!(entries[1].scope_path.as_str(), "global/project:helioy");
    assert_eq!(entries[2].scope_path.as_str(), "global");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c14_resolve_context_filters_by_kind() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:test").unwrap();
    store
        .create_scope(
            NewScope {
                path: project_path.clone(),
                label: "Test".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Fact", "fact body"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Decision", "decision body"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:test",
                EntryKind::Fact,
                "Project fact",
                "project fact body",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let entries = store
        .resolve_context(&project_path, &[EntryKind::Fact], 100)
        .await
        .unwrap();

    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.kind == EntryKind::Fact));
}
