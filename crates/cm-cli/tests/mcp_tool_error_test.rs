//! Subprocess tests for temporary MCP tool error envelopes.

mod common;

use cm_cli::mcp::MAX_MCP_RESPONSE_BYTES;
use common::{send_request, shutdown, spawn_server};
use serde_json::{Value, json};

fn assert_tool_error_workaround(resp: &Value, expected_message: &str) {
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(
        resp["error"].is_null(),
        "tool handler errors currently stay in successful tools/call results: {resp}"
    );

    let result = &resp["result"];
    assert!(
        result.get("isError").is_none(),
        "workaround must not emit top-level isError until anthropics/claude-code#22264 is fixed"
    );

    let content = result["content"].as_array().expect("content array");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    let text = content[0]["text"].as_str().expect("text content");
    assert!(
        text.starts_with("ERROR: "),
        "LLM-facing error content must keep the ERROR prefix: {text}"
    );
    assert!(
        text.contains(expected_message),
        "expected error text to contain `{expected_message}`, got `{text}`"
    );

    let meta_error = &result["_meta"]["cm_tool_error"];
    assert_eq!(meta_error["is_error"], true);
    assert_eq!(meta_error["message"], expected_message);
    assert_eq!(meta_error["suppressed_top_level_is_error"], true);
    assert_eq!(meta_error["upstream_issue"], "anthropics/claude-code#22264");
    assert!(
        meta_error["cleanup"]
            .as_str()
            .expect("cleanup note")
            .contains("isError:true"),
        "cleanup note should name the proper MCP error signal"
    );
}

#[test]
fn protocol_tools_call_unknown_tool_uses_error_workaround() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "tools/call",
            "params": {
                "name": "cx_nope",
                "arguments": {}
            }
        }),
    );

    assert_eq!(resp["id"], 100);
    assert_tool_error_workaround(&resp, "Unknown tool: cx_nope");

    shutdown(child, stdin);
}

#[test]
fn protocol_tools_call_unknown_tool_caps_error_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let tool_name = format!("cx_{}", "x".repeat(20_000));
    let resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 102,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": {}
            }
        }),
    );

    assert_eq!(resp["id"], 102);
    let serialized = serde_json::to_string(&resp).expect("serialize response");
    assert!(
        serialized.len() <= MAX_MCP_RESPONSE_BYTES,
        "full error envelope should fit inside the byte cap"
    );

    let result = &resp["result"];
    let text = result["content"][0]["text"].as_str().expect("text content");
    assert!(
        text.starts_with("ERROR: Unknown tool: cx_"),
        "LLM-facing error content must preserve the error prefix"
    );
    assert!(
        text.len() <= MAX_MCP_RESPONSE_BYTES,
        "LLM-facing error content must fit inside the byte cap"
    );
    assert!(
        text.contains("[Truncated"),
        "capped error content should carry the truncation advisory"
    );
    let meta_error = &result["_meta"]["cm_tool_error"];
    assert_eq!(meta_error["is_error"], true);
    let meta_message = meta_error["message"].as_str().expect("meta message");
    assert!(
        meta_message.contains("[Truncated"),
        "duplicated meta error message should also be capped"
    );

    shutdown(child, stdin);
}

#[test]
fn protocol_tools_call_validation_failure_uses_error_workaround() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "tools/call",
            "params": {
                "name": "cx_get",
                "arguments": {"ids": []}
            }
        }),
    );

    assert_eq!(resp["id"], 101);
    assert_tool_error_workaround(&resp, "Validation error: ids cannot be empty");

    shutdown(child, stdin);
}
