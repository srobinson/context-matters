//! Scope resolution, FTS search, browse, and pagination tests.

mod common;

use cm_core::{BrowseSort, EntryFilter, EntryKind, NewScope, Pagination};
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

// ── Sort mode tests ────────────────────────────────────────────

/// Helper: create entries with distinct titles, kinds, and scopes for sort testing.
/// Returns entry titles in creation order.
async fn create_sortable_entries(store: &CmStore) -> Vec<String> {
    create_global(store).await;

    // create_project_scope calls create_global internally, so create sub-scopes manually
    let alpha_path = ScopePath::parse("global/project:alpha").unwrap();
    store
        .create_scope(
            NewScope {
                path: alpha_path,
                label: "alpha".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    let beta_path = ScopePath::parse("global/project:beta").unwrap();
    store
        .create_scope(
            NewScope {
                path: beta_path,
                label: "beta".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let entries = [
        ("global", EntryKind::Fact, "Zebra facts"),
        ("global", EntryKind::Decision, "Alpha decision"),
        ("global/project:alpha", EntryKind::Lesson, "Middle lesson"),
        (
            "global/project:beta",
            EntryKind::Observation,
            "Beta observation",
        ),
        ("global", EntryKind::Pattern, "Delta pattern"),
    ];

    let mut titles = Vec::new();
    for (scope, kind, title) in entries {
        store
            .create_entry(
                new_entry(scope, kind, title, &format!("Body for {title}")),
                &test_ctx(),
            )
            .await
            .unwrap();
        titles.push(title.to_owned());
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    titles
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_recent() {
    let (store, _dir) = test_store().await;
    let titles = create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::Recent,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.title.as_str()).collect();
    // Most recently created last, so reversed
    let mut expected: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    expected.reverse();
    assert_eq!(got, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_oldest() {
    let (store, _dir) = test_store().await;
    let titles = create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::Oldest,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.title.as_str()).collect();
    let expected: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    assert_eq!(got, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_title_asc() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleAsc,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(
        got,
        vec![
            "Alpha decision",
            "Beta observation",
            "Delta pattern",
            "Middle lesson",
            "Zebra facts",
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_title_desc() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleDesc,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(
        got,
        vec![
            "Zebra facts",
            "Middle lesson",
            "Delta pattern",
            "Beta observation",
            "Alpha decision",
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_scope_asc() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::ScopeAsc,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.scope_path.as_str()).collect();
    // "global" < "global/project:alpha" < "global/project:beta"
    // Within same scope, secondary sort is updated_at DESC
    assert_eq!(got[0], "global");
    assert_eq!(*got.last().unwrap(), "global/project:beta");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_kind_asc() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::KindAsc,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.kind.as_str()).collect();
    // Verify ascending kind order
    for window in got.windows(2) {
        assert!(
            window[0] <= window[1],
            "Expected {0} <= {1}",
            window[0],
            window[1]
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_kind_desc() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::KindDesc,
            ..Default::default()
        })
        .await
        .unwrap();

    let got: Vec<&str> = result.items.iter().map(|e| e.kind.as_str()).collect();
    // Verify descending kind order
    for window in got.windows(2) {
        assert!(
            window[0] >= window[1],
            "Expected {0} >= {1}",
            window[0],
            window[1]
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_cursor_continuation() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    // Page through TitleAsc with limit=2
    let page1 = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleAsc,
            pagination: Pagination {
                limit: 2,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 2);
    assert!(page1.next_cursor.is_some());
    assert_eq!(page1.items[0].title, "Alpha decision");
    assert_eq!(page1.items[1].title, "Beta observation");

    let page2 = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleAsc,
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
    assert_eq!(page2.items[0].title, "Delta pattern");
    assert_eq!(page2.items[1].title, "Middle lesson");

    let page3 = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleAsc,
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
    assert_eq!(page3.items[0].title, "Zebra facts");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_cursor_mismatch_rejected() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    // Get a cursor from TitleAsc
    let page1 = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleAsc,
            pagination: Pagination {
                limit: 2,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();

    // Try using that cursor with a different sort mode
    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::Recent,
            pagination: Pagination {
                limit: 2,
                cursor: page1.next_cursor,
            },
            ..Default::default()
        })
        .await;

    assert!(result.is_err(), "Cursor sort mismatch should be rejected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_sort_oldest_cursor_continuation() {
    let (store, _dir) = test_store().await;
    let titles = create_sortable_entries(&store).await;

    let page1 = store
        .browse(EntryFilter {
            sort: BrowseSort::Oldest,
            pagination: Pagination {
                limit: 3,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 3);

    let page2 = store
        .browse(EntryFilter {
            sort: BrowseSort::Oldest,
            pagination: Pagination {
                limit: 3,
                cursor: page1.next_cursor,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page2.items.len(), 2);
    assert!(page2.next_cursor.is_none());

    // Verify no overlap and correct total order
    let all_titles: Vec<&str> = page1
        .items
        .iter()
        .chain(page2.items.iter())
        .map(|e| e.title.as_str())
        .collect();
    let expected: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    assert_eq!(all_titles, expected);
}
