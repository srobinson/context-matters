use super::support::{create_global, seed_entry, test_store};

use cm_capabilities::browse::{BROWSE_SCOPE_DEFAULT_ADVISORY, BrowseRequest, browse};
use cm_core::EntryKind;

#[tokio::test(flavor = "multi_thread")]
async fn browse_returns_all_entries_with_defaults() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact one", "Body one.", EntryKind::Fact).await;
    seed_entry(&store, "Fact two", "Body two.", EntryKind::Decision).await;

    let result = browse(
        &store,
        BrowseRequest {
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.total, 2);
    assert!(!result.has_more);
    assert!(result.next_cursor.is_none());
    assert_eq!(result.scope_used.as_deref(), Some("auto"));
    assert!(result.include_resolution);
    assert_eq!(result.limit_used, 20);
    assert_eq!(
        result.advisory.as_deref(),
        Some(BROWSE_SCOPE_DEFAULT_ADVISORY)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_returns_empty_when_no_matches() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            kind: Some(EntryKind::Decision),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(result.entries.is_empty());
    assert_eq!(result.total, 0);
    assert!(!result.has_more);
}
