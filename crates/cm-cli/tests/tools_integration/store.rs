use cm_cli::mcp::tools;
use cm_core::{ContextStore, ScopePath};
use serde_json::{Value, json};

use crate::common::{create_global, extract_stored_id, test_store};

async fn store_metadata_error(metadata: Value) -> String {
    let Some(metadata) = metadata.as_object() else {
        panic!("metadata payload must be an object");
    };
    let mut payload = serde_json::Map::from_iter([
        ("title".to_owned(), json!("Bad metadata")),
        ("body".to_owned(), json!("Body.")),
        ("kind".to_owned(), json!("fact")),
    ]);
    payload.extend(metadata.clone());

    let (store, _dir) = test_store().await;
    tools::cx_store(&store, &Value::Object(payload))
        .await
        .unwrap_err()
}

async fn update_metadata_error(metadata: Value) -> String {
    let (store, _dir) = test_store().await;
    tools::cx_update(
        &store,
        &json!({
            "id": "01950000-0000-7000-8000-000000000000",
            "meta": metadata
        }),
    )
    .await
    .unwrap_err()
}

#[tokio::test(flavor = "multi_thread")]
async fn store_creates_entry_at_global_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Test fact",
            "body": "This is a test fact body.",
            "kind": "fact"
        }),
    )
    .await;

    let text = result.unwrap().text;
    assert!(text.contains("scope: global"));
    assert!(text.contains("kind: fact"));
    // The YAML envelope carries the full uuid on its `stored:` line; the
    // helper both asserts the line exists and returns the id for reuse.
    assert!(extract_stored_id(&text).len() > 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_scope_auto_creates_scope_chain() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Repo-level decision",
            "body": "Use sqlx for database access.",
            "kind": "decision",
            "scope": "global/project:helioy/repo:nancyr"
        }),
    )
    .await;

    let text = result.unwrap().text;
    assert!(text.contains("scope: global/project:helioy/repo:nancyr"));

    // Verify ancestor scopes were created.
    let project_scope = store
        .get_scope(&ScopePath::parse("global/project:helioy").unwrap())
        .await
        .unwrap();
    assert_eq!(project_scope.label, "helioy");
}

#[tokio::test(flavor = "multi_thread")]
async fn store_with_supersedes() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r1 = tools::cx_store(
        &store,
        &json!({
            "title": "Original decision",
            "body": "Use diesel for ORM.",
            "kind": "decision"
        }),
    )
    .await
    .unwrap()
    .text;
    let old_id = extract_stored_id(&r1);

    let r2 = tools::cx_store(
        &store,
        &json!({
            "title": "Updated decision",
            "body": "Use sqlx instead of diesel.",
            "kind": "decision",
            "supersedes": &old_id
        }),
    )
    .await
    .unwrap()
    .text;
    // The ack carries `superseded: <old_id>` right after the new `stored:`
    // line when `supersedes` was passed on the request.
    assert!(r2.contains(&format!("superseded: {old_id}")));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_empty_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "",
            "body": "Some body",
            "kind": "fact"
        }),
    )
    .await;
    assert!(result.is_err() || result.unwrap().text.contains("empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_invalid_kind() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Test",
            "body": "Test body",
            "kind": "bogus"
        }),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid kind"));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_and_update_share_invalid_metadata_errors() {
    let cases = [
        json!({ "confidence": "maybe" }),
        json!({ "expires_at": "not-a-date" }),
        json!({ "tags": ["valid", 42] }),
    ];

    for metadata in cases {
        let store_error = store_metadata_error(metadata.clone()).await;
        let update_error = update_metadata_error(metadata).await;

        assert_eq!(store_error, update_error);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_invalid_scope() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Test",
            "body": "Test body",
            "kind": "fact",
            "scope": "not/valid"
        }),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid scope"));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_detects_duplicate_content() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let args = json!({
        "title": "Dup test",
        "body": "Identical body content.",
        "kind": "fact"
    });

    tools::cx_store(&store, &args).await.unwrap();
    let result = tools::cx_store(&store, &args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Duplicate content"));
}
