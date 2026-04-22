use super::support::{create_global, seed_superseded_fact_pair, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};

#[tokio::test(flavor = "multi_thread")]
async fn browse_excludes_superseded_by_default() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_superseded_fact_pair(&store).await;

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
    assert_eq!(result.entries[0].title, "Replacement");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_includes_superseded_when_opted_in() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_superseded_fact_pair(&store).await;

    let result = browse(
        &store,
        BrowseRequest {
            include_superseded: true,
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 2);
}
