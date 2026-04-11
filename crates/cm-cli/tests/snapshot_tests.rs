//! Insta snapshot tests for all 9 `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp-file SQLite database, calls a tool handler,
//! and snapshots the JSON response with dynamic fields redacted. This catches
//! unintentional changes to the response format that would break MCP clients.

mod common;

use cm_cli::mcp::tools;
use cm_store::CmStore;
use insta::{assert_json_snapshot, with_settings};
use serde_json::{Value, json};

use common::{create_global, extract_stored_id, test_store};

/// Store a test entry and return its ID. The `cx_store` handler now returns
/// a YAML text envelope, so the id is scraped from the `stored:` line via
/// [`extract_stored_id`] instead of being parsed out of a JSON blob.
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
    extract_stored_id(&result)
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
// cx_store now returns YAML text, which is incompatible with
// `assert_json_snapshot!`. Sub 13 migrates this test to either
// `assert_snapshot!` (raw text) or deletes it in favour of the
// formatter-side snapshots already living in cm-capabilities.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
// cx_recall now returns YAML text, which is incompatible with
// `assert_json_snapshot!`. Sub 13 migrates this test to either
// `assert_snapshot!` (raw text) or deletes it in favour of the
// formatter-side snapshots already living in cm-capabilities.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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

// TODO(ALP-1738, sub 13): rebaseline for YAML-text envelope.
#[ignore = "rebaseline in ALP-1738 sub 13"]
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
