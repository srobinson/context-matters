//! Capability-level tests for update validation and store access.

use cm_capabilities::update::{UpdateRequest, update as update_entry};
use cm_core::{
    CmError, Confidence, ContextStore, EntryKind, MutationSource, NewEntry, NewScope, ScopePath,
    WriteContext,
};
use cm_store::{CmStore, schema};
use serde_json::json;

async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::global(),
                label: "Global".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_entry(store: &CmStore) -> cm_core::Entry {
    create_global(store).await;
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind: EntryKind::Fact,
                title: "Original".to_owned(),
                body: "Original body.".to_owned(),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap()
}

fn request(value: serde_json::Value) -> UpdateRequest {
    serde_json::from_value(value).unwrap()
}

fn assert_validation(err: CmError, expected: &str) {
    match err {
        CmError::Validation(msg) => assert_eq!(msg, expected),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn update_changes_title_body_and_reports_ack_data() {
    let (store, _dir) = test_store().await;
    let entry = seed_entry(&store).await;

    let result = update_entry(
        &store,
        request(json!({
            "id": entry.id.to_string(),
            "title": "Updated",
            "body": "Updated body."
        })),
        &wctx(),
    )
    .await
    .unwrap();

    assert_eq!(result.updated_id, entry.id.to_string());
    assert_eq!(result.content_hash.len(), 64);

    let updated = store.get_entry(entry.id).await.unwrap();
    assert_eq!(updated.title, "Updated");
    assert_eq!(updated.body, "Updated body.");
}

#[tokio::test(flavor = "multi_thread")]
async fn update_canonicalizes_uuid_input() {
    let (store, _dir) = test_store().await;
    let entry = seed_entry(&store).await;
    let uppercase_simple_id = entry.id.simple().to_string().to_uppercase();

    let result = update_entry(
        &store,
        request(json!({
            "id": uppercase_simple_id,
            "title": "Canonical"
        })),
        &wctx(),
    )
    .await
    .unwrap();

    assert_eq!(result.updated_id, entry.id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn update_rejects_invalid_uuid_before_writing() {
    let (store, _dir) = test_store().await;
    let err = update_entry(
        &store,
        request(json!({
            "id": "not-a-uuid",
            "title": "Ignored"
        })),
        &wctx(),
    )
    .await
    .unwrap_err();

    match err {
        CmError::Validation(msg) => assert!(msg.contains("invalid UUID 'not-a-uuid'")),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn update_rejects_no_fields() {
    let (store, _dir) = test_store().await;
    let err = update_entry(
        &store,
        request(json!({
            "id": "01950000-0000-7000-8000-000000000000"
        })),
        &wctx(),
    )
    .await
    .unwrap_err();

    assert_validation(
        err,
        "at least one field must be provided (--title, --body, --kind, --meta)",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_rejects_oversized_body() {
    let (store, _dir) = test_store().await;
    let err = update_entry(
        &store,
        UpdateRequest {
            id: "01950000-0000-7000-8000-000000000000".to_owned(),
            title: None,
            body: Some("x".repeat(cm_capabilities::constants::MAX_INPUT_BYTES + 1)),
            kind: None,
            meta: None,
        },
        &wctx(),
    )
    .await
    .unwrap_err();

    assert_validation(
        err,
        &format!(
            "body exceeds {} byte limit",
            cm_capabilities::constants::MAX_INPUT_BYTES
        ),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_rejects_invalid_kind() {
    let (store, _dir) = test_store().await;
    let entry = seed_entry(&store).await;
    let err = update_entry(
        &store,
        request(json!({
            "id": entry.id.to_string(),
            "kind": "idea"
        })),
        &wctx(),
    )
    .await
    .unwrap_err();

    assert_validation(
        err,
        "Invalid kind 'idea'. Valid values: fact, decision, preference, lesson, reference, feedback, pattern, observation.",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_persists_metadata_via_meta_input() {
    let (store, _dir) = test_store().await;
    let entry = seed_entry(&store).await;

    update_entry(
        &store,
        request(json!({
            "id": entry.id.to_string(),
            "meta": {
                "tags": ["dx", "adapter"],
                "confidence": "high",
                "source": "https://example.test/context",
                "priority": 7
            }
        })),
        &wctx(),
    )
    .await
    .unwrap();

    let updated = store.get_entry(entry.id).await.unwrap();
    let meta = updated.meta.unwrap();
    assert_eq!(meta.tags, vec!["dx", "adapter"]);
    assert_eq!(meta.confidence, Some(Confidence::High));
    assert_eq!(meta.source.as_deref(), Some("https://example.test/context"));
    assert_eq!(meta.priority, Some(7));
}
