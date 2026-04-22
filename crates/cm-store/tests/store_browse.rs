//! Browse pagination and filter tests.

mod common;

use cm_core::{EntryFilter, EntryKind, EntryMeta, Pagination};
use common::*;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_tag_filter_matches_exact_json_array_element() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let mut exact = new_entry(
        "global",
        EntryKind::Fact,
        "Exact wildcard tag",
        "Body with literal wildcard tag",
    );
    exact.meta = Some(EntryMeta {
        tags: vec!["rust_%".to_owned()],
        ..Default::default()
    });
    store.create_entry(exact, &test_ctx()).await.unwrap();

    let mut wildcard_match = new_entry(
        "global",
        EntryKind::Fact,
        "LIKE wildcard false positive",
        "Body with tag that LIKE would match",
    );
    wildcard_match.meta = Some(EntryMeta {
        tags: vec!["rust_async".to_owned()],
        ..Default::default()
    });
    store
        .create_entry(wildcard_match, &test_ctx())
        .await
        .unwrap();

    let result = store
        .browse(EntryFilter {
            tag: Some("rust_%".to_owned()),
            pagination: Pagination {
                limit: 10,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total, 1);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Exact wildcard tag");
}
