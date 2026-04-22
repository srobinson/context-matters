use serde_json::json;

use super::support::{get_json, seed_entries, test_app, test_store};

// Pin the field names on the projection shapes so a rename in
// `web_view.rs` cannot silently break the wire contract.

#[tokio::test(flavor = "multi_thread")]
async fn recall_header_has_required_fields() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/agent/recall?query=architecture&kinds=fact&tags=architecture",
    )
    .await;

    let header = web.get("header").expect("header must be present");
    for field in [
        "query",
        "routing",
        "candidates",
        "returned",
        "scope_chain",
        "scope_hits",
        "kinds_histogram",
        "tags_histogram",
        "tokens",
    ] {
        assert!(
            header.get(field).is_some(),
            "header.{field} required on WebRecallView"
        );
    }
    assert_eq!(header["query"], json!("architecture"));

    assert!(web.get("entries").is_some(), "entries array required");
    assert!(web.get("advisories").is_some(), "advisories array required");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_header_has_required_fields() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?kind=fact&tag=architecture").await;

    let header = web.get("header").expect("header must be present");
    for field in [
        "sort_used",
        "total",
        "returned",
        "kinds_histogram",
        "tags_histogram",
    ] {
        assert!(
            header.get(field).is_some(),
            "header.{field} required on WebBrowseView"
        );
    }
    assert_eq!(header["sort_used"], json!("updated_at desc"));

    assert!(web.get("entries").is_some(), "entries array required");
    assert!(web.get("has_more").is_some(), "has_more flag required");
}
