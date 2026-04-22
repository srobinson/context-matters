use cm_cli::mcp::tools;
use serde_json::json;

use crate::common::{create_global, extract_stored_id, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn forget_soft_deletes_entry() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "To forget", "body": "Will be forgotten.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    let result = tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    // `format_forget_ack` renders the three disposition counters
    // unconditionally, each on its own line.
    assert!(result.contains("forgotten: 1"));
    assert!(result.contains("already_inactive: 0"));

    // Recall searches by short id prefix against the rendered row list.
    let recall = tools::cx_recall(&store, &json!({})).await.unwrap().text;
    let sid_prefix = &id[..8];
    assert!(!recall.contains(sid_prefix));
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_reports_already_inactive() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Double forget", "body": "Body.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap();
    let result = tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(result.contains("forgotten: 0"));
    assert!(result.contains("already_inactive: 1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_reports_not_found() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_forget(
        &store,
        &json!({"ids": ["01950000-0000-7000-8000-000000000000"]}),
    )
    .await
    .unwrap()
    .text;
    assert!(result.contains("not_found: 1"));
}
