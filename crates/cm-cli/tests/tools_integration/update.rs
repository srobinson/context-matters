use cm_cli::mcp::tools;
use serde_json::json;

use crate::common::{create_global, extract_stored_id, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn update_changes_title_and_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Original", "body": "Original body.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    let result = tools::cx_update(
        &store,
        &json!({
            "id": &id,
            "title": "Updated title",
            "body": "Updated body content."
        }),
    )
    .await
    .unwrap()
    .text;
    // `format_update_ack` emits just `updated: <id>` + `content_hash: <prefix>`
    // by design, scope/kind never change, title lives in the entry body.
    // The body/title round trip is covered by `e2e_store_update_verify`.
    assert!(result.contains(&format!("updated: {id}")));
    assert!(result.contains("content_hash: "));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_rejects_no_fields() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_update(
        &store,
        &json!({"id": "01950000-0000-7000-8000-000000000000"}),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least one field"));
}
