mod common;

use cm_capabilities::{ContentSearchRequest, search::search};
use cm_core::{CmError, EntryKind, ScopeFilter};
use common::{create_global, seed_entry, test_store};

fn search_request(query: &str) -> ContentSearchRequest {
    ContentSearchRequest {
        query: query.to_owned(),
        scope: ScopeFilter::All,
        kinds: None,
        tags: None,
        limit: 10,
        cursor: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_rejects_empty_query_with_browse_hint() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let err = search(&store, search_request("   ")).await.unwrap_err();

    match err {
        CmError::InvalidOperationInput { op, reason } => {
            assert_eq!(op, "cx_search");
            assert!(reason.contains("query"));
            assert!(reason.contains("cx_browse"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_rejects_operator_only_queries_as_invalid_input() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for query in ["AND", "OR", "NOT", "AND OR NOT"] {
        let err = search(&store, search_request(query)).await.unwrap_err();

        match err {
            CmError::InvalidOperationInput { op, reason } => {
                assert_eq!(op, "cx_search", "{query}");
                assert!(reason.contains("query"), "{query}: {reason}");
            }
            other => panic!("{query}: unexpected error: {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_maps_fts_parse_errors_to_invalid_input() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Content search note",
        "alpha search capability body",
        EntryKind::Fact,
    )
    .await;

    let err = search(&store, search_request("alpha OR"))
        .await
        .unwrap_err();

    match err {
        CmError::InvalidOperationInput { op, reason } => {
            assert_eq!(op, "cx_search");
            assert!(reason.contains("query"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_returns_content_search_page() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Content search note",
        "alpha search capability body",
        EntryKind::Fact,
    )
    .await;

    let page = search(&store, search_request("alpha")).await.unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].entry.title, "Content search note");
    assert!(page.items[0].score.is_finite());
}
