//! Insta snapshot test for `cx_export`.
//!
//! Eight of the nine handler-level snapshots landed in the initial
//! MCP-tools test suite were retired as part of the YAML-payload
//! migration (ALP-1725). Their coverage now lives at two tighter
//! layers:
//!
//! - Formatter-level goldens in `cm-capabilities/tests/*_format_tests.rs`
//!   (include_str! + assert_eq!) pin the exact rendered shape for
//!   each `format_*_view` / `format_*_ack`.
//! - Handler-level YAML substring assertions in `tools_integration.rs`
//!   exercise the full handler → formatter → `yaml_response` →
//!   `cap_response` pipeline with realistic fixtures.
//!
//! The sole survivor is `cx_export`, which stays on `json_response`
//! (JSON fidelity > wire compactness for backup/restore). Post
//! ALP-1760, `json_response` produces a structured-only `ToolResult`
//! and the envelope builder surfaces it as `structuredContent` with
//! `content: []`; the insta snapshot asserts against the structured
//! channel directly instead of parsing a serialised JSON text blob.
//!
//! See [`crates/cm-cli/tests/common/mod.rs`] for the shared fixture
//! and id-extraction helpers.

mod common;

use cm_cli::mcp::tools;
use insta::{assert_json_snapshot, with_settings};
use serde_json::{Value, json};

use common::{create_global, test_store};

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

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_cx_export() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
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

    let result = tools::cx_export(&store, &json!({"format": "json"}))
        .await
        .unwrap();

    // ALP-1760: `cx_export` now returns a structured-only `ToolResult`.
    // Pull the JSON out of `result.structured` directly — the envelope
    // builder surfaces it as `structuredContent` with an empty `content`
    // array, and the snapshot asserts against the same JSON value the
    // wire would carry.
    let mut resp: Value = result
        .structured
        .expect("cx_export must emit a structured payload");
    redact_dynamic_fields(&mut resp);

    snapshot_settings! {
        assert_json_snapshot!("cx_export", resp);
    }
}

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

/// Keys whose values change every run and must be redacted. Scoped
/// to the fields `cx_export` emits; the broader set that previously
/// covered the retired handler snapshots is no longer needed.
fn is_redacted_key(key: &str) -> bool {
    matches!(
        key,
        "id" | "created_at" | "updated_at" | "content_hash" | "exported_at"
    )
}
