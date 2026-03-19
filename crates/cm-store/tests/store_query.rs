//! Scope resolution, FTS search, browse, and pagination tests.

mod common;

use cm_core::{EntryFilter, EntryKind, Pagination};
use common::*;

// ── Scope-based query ───────────────────────────────────────────

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

// ── Resolve context (ancestor walk) ─────────────────────────────

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

// ── FTS search ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c19_fts_search_finds_by_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Photosynthesis in plants",
                "Some body content",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("photosynthesis", None, 10).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Photosynthesis in plants");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c19_fts_search_finds_by_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Generic title",
                "The mitochondria is the powerhouse of the cell",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("mitochondria", None, 10).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Generic title");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c20_fts_reflects_updated_content() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Interesting title",
                "Original content about elephants",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("Updated content about giraffes".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("giraffes", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);

    let results = store.search("elephants", None, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c21_superseded_entries_excluded_from_search() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Searchable entry",
                "Contains the word quantum",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("quantum", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);

    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    let results = store.search("quantum", None, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c25_updated_at_and_fts_both_fire_on_update() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Dual trigger test",
                "Original content keyword: albatross",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let updated = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("New content keyword: pelican".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert!(updated.updated_at > entry.updated_at);

    let found = store.search("pelican", None, 10).await.unwrap();
    assert_eq!(found.len(), 1);

    let not_found = store.search("albatross", None, 10).await.unwrap();
    assert_eq!(not_found.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_with_scope_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:scoped").unwrap();
    store
        .create_scope(
            NewScope {
                path: project_path.clone(),
                label: "Scoped".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global/project:other").unwrap(),
                label: "Other".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry(
                "global/project:scoped",
                EntryKind::Fact,
                "Scoped entry",
                "Contains the word butterfly",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:other",
                EntryKind::Fact,
                "Other entry",
                "Also contains butterfly",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Global entry",
                "Global butterfly too",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store
        .search("butterfly", Some(&project_path), 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    let scopes: Vec<&str> = results.iter().map(|e| e.scope_path.as_str()).collect();
    assert!(scopes.contains(&"global/project:scoped"));
    assert!(scopes.contains(&"global"));
    assert!(!scopes.contains(&"global/project:other"));
}

// ── Browse pagination ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_pagination_with_cursor() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..5 {
        store
            .create_entry(
                new_entry(
                    "global",
                    EntryKind::Fact,
                    &format!("Entry {i}"),
                    &format!("Body {i}"),
                ),
                &test_ctx(),
            )
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let page1 = store
        .browse(EntryFilter {
            pagination: Pagination {
                limit: 2,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.total, 5);
    assert!(page1.next_cursor.is_some());

    let page2 = store
        .browse(EntryFilter {
            pagination: Pagination {
                limit: 2,
                cursor: page1.next_cursor,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page2.items.len(), 2);
    assert!(page2.next_cursor.is_some());

    let page3 = store
        .browse(EntryFilter {
            pagination: Pagination {
                limit: 2,
                cursor: page2.next_cursor,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page3.items.len(), 1);
    assert!(page3.next_cursor.is_none());

    let all_ids: Vec<_> = page1
        .items
        .iter()
        .chain(page2.items.iter())
        .chain(page3.items.iter())
        .map(|e| e.id)
        .collect();
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(all_ids.len(), unique.len(), "Pages should not overlap");
}
