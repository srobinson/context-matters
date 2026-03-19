//! Insta snapshot tests for all 9 `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp-file SQLite database, calls a tool handler,
//! and snapshots the JSON response with dynamic fields redacted. This catches
//! unintentional changes to the response format that would break MCP clients.

use cm_cli::mcp::tools;
use cm_core::{ContextStore, MutationSource, NewScope, ScopePath, WriteContext};
use cm_store::{CmStore, schema};
use insta::{assert_json_snapshot, with_settings};
use serde_json::{Value, json};

/// Create an isolated store backed by a temp-file SQLite database.
async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();

    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

/// Create the global scope.
async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Global".to_owned(),
                meta: None,
            },
            &WriteContext::new(MutationSource::Mcp),
        )
        .await
        .unwrap();
}

/// Store a test entry and return its ID.
async fn store_entry(store: &CmStore) -> String {
    let result = tools::cx_store(
        store,
        &json!({
            "title": "Test fact",
            "body": "This is a test fact body for snapshot testing.",
            "kind": "fact",
            "tags": ["test-tag"],
            "confidence": "high"
        }),
    )
    .await
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    resp["id"].as_str().unwrap().to_owned()
}

/// Redaction settings for dynamic fields that change every run.
macro_rules! snapshot_settings {
    ($($body:tt)*) => {
        with_settings!({
            // Sort maps for deterministic output
            sort_maps => true,
        }, {
            $($body)*
        })
    };
}

// ── cx_store ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_store() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Architecture decision",
            "body": "Use sqlx for database access.",
            "kind": "decision",
            "tags": ["architecture", "database"],
            "confidence": "high"
        }),
    )
    .await
    .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_store", resp);
    }
}

// ── cx_recall ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_recall() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    store_entry(&store).await;

    let result = tools::cx_recall(
        &store,
        &json!({
            "query": "test fact",
            "limit": 5
        }),
    )
    .await
    .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_recall", resp);
    }
}

// ── cx_get ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_get() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let id = store_entry(&store).await;

    let result = tools::cx_get(&store, &json!({"ids": [id]})).await.unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_get", resp);
    }
}

// ── cx_browse ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_browse() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    store_entry(&store).await;

    let result = tools::cx_browse(&store, &json!({"limit": 10}))
        .await
        .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_browse", resp);
    }
}

// ── cx_update ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_update() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let id = store_entry(&store).await;

    let result = tools::cx_update(
        &store,
        &json!({
            "id": id,
            "title": "Updated title",
            "body": "Updated body content."
        }),
    )
    .await
    .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_update", resp);
    }
}

// ── cx_forget ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_forget() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let id = store_entry(&store).await;

    let result = tools::cx_forget(&store, &json!({"ids": [id]}))
        .await
        .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_forget", resp);
    }
}

// ── cx_deposit ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_deposit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_deposit(
        &store,
        &json!({
            "exchanges": [
                {
                    "user": "What is context-matters?",
                    "assistant": "A structured context store for AI agents."
                }
            ],
            "summary": "Brief intro to context-matters."
        }),
    )
    .await
    .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_deposit", resp);
    }
}

// ── cx_export ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_export() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    store_entry(&store).await;

    let result = tools::cx_export(&store, &json!({"format": "json"}))
        .await
        .unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_export", resp);
    }
}

// ── cx_stats ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_stats() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    store_entry(&store).await;

    let result = tools::cx_stats(&store, &json!({})).await.unwrap();

    let mut resp: Value = serde_json::from_str(&result).unwrap();
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_stats", resp);
    }
}

// ── Redaction helpers ──────────────────────────────────────────

/// Recursively redact dynamic fields that change every run.
fn redact_dynamic_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_redacted_key(key) {
                    *val = Value::String("[redacted]".to_owned());
                } else {
                    redact_dynamic_fields(val);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                redact_dynamic_fields(item);
            }
        }
        _ => {}
    }
}

/// Keys whose values change every run and must be redacted.
fn is_redacted_key(key: &str) -> bool {
    matches!(
        key,
        "id" | "created_at"
            | "updated_at"
            | "content_hash"
            | "db_size_bytes"
            | "exported_at"
            | "entry_ids"
            | "summary_id"
    )
}
