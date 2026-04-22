//! Parity tests for metadata validation shared by store and update.

use cm_capabilities::{
    store::{StoreRequest, store as store_entry},
    update::{UpdateRequest, update as update_entry},
};
use cm_core::{CmError, MutationSource, WriteContext};
use cm_store::{CmStore, schema};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

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

fn deserialize_error<T: DeserializeOwned>(value: Value) -> Option<String> {
    serde_json::from_value::<T>(value)
        .err()
        .map(|err| err.to_string())
}

fn validation_message(err: CmError) -> String {
    match err {
        CmError::Validation(message) => message,
        other => panic!("expected validation error, got {other:?}"),
    }
}

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
    let payload = Value::Object(payload);

    // Malformed tag arrays fail while building the shared capability request.
    // The other metadata cases reach MetaInput::into_entry_meta below.
    if let Some(message) = deserialize_error::<StoreRequest>(payload.clone()) {
        return message;
    }

    let request = serde_json::from_value(payload).unwrap();
    let (store, _dir) = test_store().await;
    let err = store_entry(&store, request, &wctx()).await.unwrap_err();
    validation_message(err)
}

async fn update_metadata_error(metadata: Value) -> String {
    let payload = json!({
        "id": "01950000-0000-7000-8000-000000000000",
        "meta": metadata
    });

    // Keep this in step with store_metadata_error so request-shape failures
    // and validation failures are compared through the same adapter boundary.
    if let Some(message) = deserialize_error::<UpdateRequest>(payload.clone()) {
        return message;
    }

    let request = serde_json::from_value(payload).unwrap();
    let (store, _dir) = test_store().await;
    let err = update_entry(&store, request, &wctx()).await.unwrap_err();
    validation_message(err)
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
