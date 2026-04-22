//! Browse sort and sort cursor tests.

mod common;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cm_core::{BrowseSort, EntryFilter, EntryKind, NewScope, Pagination};
use common::*;
use serde_json::json;

async fn create_sortable_entries(store: &CmStore) -> Vec<String> {
    create_global(store).await;

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
async fn browse_text_sort_cursor_missing_primary_value_rejected() {
    let (store, _dir) = test_store().await;
    create_sortable_entries(&store).await;

    let malformed_cursor = URL_SAFE_NO_PAD.encode(
        json!({
            "sort": "title_asc",
            "ts": "2026-01-01T00:00:00.000Z",
            "id": "00000000-0000-0000-0000-000000000000"
        })
        .to_string(),
    );

    let result = store
        .browse(EntryFilter {
            sort: BrowseSort::TitleAsc,
            pagination: Pagination {
                limit: 2,
                cursor: Some(malformed_cursor),
            },
            ..Default::default()
        })
        .await;

    assert!(
        result.is_err(),
        "Text sort cursor missing primary value should be rejected"
    );
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

    let all_titles: Vec<&str> = page1
        .items
        .iter()
        .chain(page2.items.iter())
        .map(|e| e.title.as_str())
        .collect();
    let expected: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
    assert_eq!(all_titles, expected);
}
