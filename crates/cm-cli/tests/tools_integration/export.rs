use cm_cli::mcp::tools;
use serde_json::{Value, json};

use crate::common::{create_global, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn export_returns_all_entries() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "Export test", "body": "Body.", "kind": "fact"}),
    )
    .await
    .unwrap();

    let result = tools::cx_export(&store, &json!({})).await.unwrap();
    // cx_export emits structured only: text channel is empty, the JSON
    // payload lives in the structured channel for backup/restore fidelity.
    let resp: Value = result
        .structured
        .expect("cx_export must emit a structured payload");
    assert_eq!(resp["count"], 1);
    assert!(resp["exported_at"].as_str().is_some());
    assert!(!resp["scopes"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn export_rejects_unsupported_format() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_export(&store, &json!({"format": "csv"})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unsupported export format"));
}
