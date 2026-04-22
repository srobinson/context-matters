use super::support::{create_global, seed_entry, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_core::{BrowseSort, EntryKind};

#[tokio::test(flavor = "multi_thread")]
async fn browse_populates_sort_used_default() {
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

    assert_eq!(result.sort_used, BrowseSort::Recent);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_populates_sort_used_explicit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Only entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            sort: BrowseSort::Oldest,
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.sort_used, BrowseSort::Oldest);
}
