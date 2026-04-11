//! Subprocess MCP protocol tests.
//!
//! Spawn the `cm serve` binary, pipe JSON-RPC messages to stdin, assert on stdout.
//! Each test uses an isolated tempdir via `CM_DATA_DIR` to prevent cross-test interference.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use common::extract_stored_id;
use serde_json::{Value, json};

/// Path to the compiled `cm` binary under the target directory.
fn cm_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("cm")
}

/// Spawn `cm serve` with an isolated data directory and return (child, stdin, stdout_reader).
fn spawn_server(
    dir: &tempfile::TempDir,
) -> (
    std::process::Child,
    std::process::ChildStdin,
    BufReader<std::process::ChildStdout>,
) {
    let mut child = Command::new(cm_bin())
        .arg("serve")
        .env("CM_DATA_DIR", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn cm serve");

    let stdin = child.stdin.take().expect("no stdin");
    let stdout = BufReader::new(child.stdout.take().expect("no stdout"));
    (child, stdin, stdout)
}

/// Send a JSON-RPC request and read the response line.
fn send_request(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    request: &Value,
) -> Value {
    let line = serde_json::to_string(request).unwrap();
    writeln!(stdin, "{line}").expect("write to stdin");
    stdin.flush().expect("flush stdin");

    let mut response_line = String::new();
    stdout
        .read_line(&mut response_line)
        .expect("read from stdout");
    serde_json::from_str(&response_line).expect("parse JSON response")
}

/// Gracefully close the server by dropping stdin (EOF) and waiting.
fn shutdown(mut child: std::process::Child, stdin: std::process::ChildStdin) {
    drop(stdin);
    let _ = child.wait();
}

// ── Test 1: Initialize handshake ───────────────────────────────

#[test]
fn protocol_initialize_handshake() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    let resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0.1.0"}
            }
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(resp["error"].is_null());

    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "cm");
    assert!(result["serverInfo"]["version"].is_string());
    assert!(result["instructions"].is_string());
    assert!(result["capabilities"]["tools"].is_object());

    shutdown(child, stdin);
}

// ── Test 2: Tools list ─────────────────────────────────────────

#[test]
fn protocol_tools_list() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    // Initialize first
    send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
    );

    let resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 2);
    assert!(resp["error"].is_null());

    let tools = resp["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 9, "expected 9 MCP tools");

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    for expected in &[
        "cx_recall",
        "cx_store",
        "cx_deposit",
        "cx_browse",
        "cx_get",
        "cx_update",
        "cx_forget",
        "cx_stats",
        "cx_export",
    ] {
        assert!(tool_names.contains(expected), "missing tool: {expected}");
    }

    // Each tool should have name, description, and inputSchema
    for tool in tools {
        assert!(tool["name"].is_string(), "tool missing name");
        assert!(tool["description"].is_string(), "tool missing description");
        assert!(tool["inputSchema"].is_object(), "tool missing inputSchema");
    }

    // ALP-1759: read tools advertise outputSchema so MCP 2025-06-18 clients can
    // validate structuredContent. Write tools stay text-only (no structured
    // receipt payload to describe).
    let read_tools = ["cx_recall", "cx_browse", "cx_get", "cx_stats", "cx_export"];
    let write_tools = ["cx_store", "cx_deposit", "cx_update", "cx_forget"];
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let has_output_schema = tool.get("outputSchema").is_some_and(|v| v.is_object());
        if read_tools.contains(&name) {
            assert!(has_output_schema, "read tool {name} missing outputSchema");
        } else if write_tools.contains(&name) {
            assert!(
                !has_output_schema,
                "write tool {name} unexpectedly declares outputSchema"
            );
        }
    }

    shutdown(child, stdin);
}

// ── Test 3: tools/call cx_stats ────────────────────────────────

#[test]
fn protocol_tools_call_cx_stats() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    // Initialize
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
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "cx_stats",
                "arguments": {}
            }
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 2);
    assert!(resp["error"].is_null());

    let result = &resp["result"];
    let content = result["content"].as_array().expect("content array");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");

    // `format_stats_view` emits counters as top-level YAML keys
    // (`active:`, `scopes:`, `relations:`). An empty store shows zeros
    // on every line. The `---` prefix is the YAML document marker.
    let stats_text = content[0]["text"].as_str().unwrap();
    assert!(stats_text.starts_with("---\n"));
    assert!(stats_text.contains("active: 0"));
    assert!(stats_text.contains("scopes: 0"));
    assert!(stats_text.contains("relations: 0"));

    shutdown(child, stdin);
}

// ── Test 4: Unknown method error ───────────────────────────────

#[test]
fn protocol_unknown_method() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    // Initialize
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
            "id": 99,
            "method": "bogus/method",
            "params": {}
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 99);
    assert!(resp["result"].is_null());

    let error = &resp["error"];
    assert_eq!(error["code"], -32601);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("Method not found"),
        "expected 'Method not found' in error message"
    );

    shutdown(child, stdin);
}

// ── Test 5: Store and recall roundtrip ─────────────────────────

#[test]
fn protocol_store_and_recall_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    // Initialize
    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    // Store an entry
    let store_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "cx_store",
                "arguments": {
                    "title": "Protocol test fact",
                    "body": "Testing the full store-and-recall roundtrip via MCP protocol.",
                    "kind": "fact"
                }
            }
        }),
    );

    assert!(store_resp["error"].is_null(), "store failed: {store_resp}");
    // `format_store_ack` returns a YAML envelope; `extract_stored_id`
    // scrapes the `stored: <uuid>` line so the recall half can match
    // the short-id prefix back against the rendered row list.
    let store_text = store_resp["result"]["content"][0]["text"].as_str().unwrap();
    let stored_id = extract_stored_id(store_text);
    assert!(store_text.contains("scope: global"));
    assert!(store_text.contains("kind: fact"));

    // Recall the entry
    let recall_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "cx_recall",
                "arguments": {
                    "query": "protocol test roundtrip"
                }
            }
        }),
    );

    assert!(
        recall_resp["error"].is_null(),
        "recall failed: {recall_resp}"
    );
    // `format_recall_view` surfaces `routing: search` in the header
    // and renders one entry row per hit. Rows carry the 8-char short
    // id prefix, so the roundtrip check substring-matches the first
    // eight bytes of the stored uuid against the recall body.
    let recall_text = recall_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(recall_text.contains("routing: search"));
    assert!(recall_text.contains("Protocol test fact"));
    assert!(recall_text.contains(&stored_id[..8]));

    shutdown(child, stdin);
}
