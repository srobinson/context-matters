use super::support::{create_global, seed_entry, seed_numbered_entries, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_core::EntryKind;

#[tokio::test(flavor = "multi_thread")]
async fn browse_pagination_with_cursor() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_numbered_entries(&store, 5).await;

    let page1 = browse(
        &store,
        BrowseRequest {
            limit: Some(2),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page1.entries.len(), 2);
    assert!(page1.has_more);
    assert!(page1.next_cursor.is_some());
    assert_eq!(page1.total, 5);

    let page2 = browse(
        &store,
        BrowseRequest {
            limit: Some(2),
            cursor: page1.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page2.entries.len(), 2);
    assert!(page2.has_more);

    let page3 = browse(
        &store,
        BrowseRequest {
            limit: Some(2),
            cursor: page2.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page3.entries.len(), 1);
    assert!(!page3.has_more);
    assert!(page3.next_cursor.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_respects_limit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_numbered_entries(&store, 10).await;

    let result = browse(
        &store,
        BrowseRequest {
            limit: Some(3),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 3);
    assert_eq!(result.total, 10);
    assert!(result.has_more);
}

#[tokio::test(flavor = "multi_thread")]
async fn has_more_false_when_all_returned() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Only entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert!(!result.has_more);
    assert!(result.next_cursor.is_none());
}
