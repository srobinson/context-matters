use crate::common::{send_request, shutdown, spawn_server};
use serde_json::json;

/// Assert that `cx_export` emits an empty `content` array and a
/// populated `structuredContent` payload. `cx_export` is the only
/// tool whose canonical form is JSON (backup/restore fidelity beats
/// wire compactness), so the envelope builder emits `content: []`
/// and surfaces the JSON via `structuredContent` instead. Reading
/// `content[0]["type"]` on this response would panic on an empty
/// array; the assertions use `content.as_array().is_empty()` and
/// key-lookups on the structured channel instead.
#[test]
fn protocol_tools_call_cx_export() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    // Seed one entry so the export payload has something non-empty
    // in the structured `entries` array; empty stores would pass
    // every assertion trivially.
    send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "cx_store",
                "arguments": {
                    "title": "Export test fact",
                    "body": "Seed row so the export entries array is non-empty.",
                    "kind": "fact"
                }
            }
        }),
    );

    let resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "cx_export",
                "arguments": {"format": "json"}
            }
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 3);
    assert!(resp["error"].is_null(), "cx_export failed: {resp}");

    let result = &resp["result"];
    let content = result["content"]
        .as_array()
        .expect("cx_export result must carry a (possibly empty) content array");
    assert!(
        content.is_empty(),
        "cx_export text channel must be empty; got: {content:?}"
    );

    // The JSON export lives on the structured channel. Required keys
    // match the outputSchema declared in tools.toml for cx_export:
    // entries, scopes, exported_at, count.
    let structured = &result["structuredContent"];
    assert!(
        structured.is_object(),
        "cx_export response must carry structuredContent"
    );
    for key in &["entries", "scopes", "exported_at", "count"] {
        assert!(
            structured.get(key).is_some(),
            "cx_export structuredContent missing {key}"
        );
    }
    assert!(
        structured["entries"].is_array(),
        "cx_export entries must be an array"
    );
    assert!(
        structured["scopes"].is_array(),
        "cx_export scopes must be an array"
    );
    assert!(
        structured["exported_at"].is_string(),
        "cx_export exported_at must be an ISO timestamp string"
    );
    // One seed row was stored above, so the count/entries should
    // both be 1. This catches regressions where the export handler
    // silently drops the structured payload and the envelope builder
    // falls back to an empty `structuredContent`.
    assert_eq!(structured["count"], 1);
    assert_eq!(structured["entries"].as_array().unwrap().len(), 1);

    shutdown(child, stdin);
}
