//! Subprocess MCP protocol tests.
//!
//! Spawn the `cm serve` binary, pipe JSON-RPC messages to stdin, assert on stdout.
//! Each test uses an isolated tempdir via `CM_DATA_DIR` to prevent cross-test interference.

mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use common::{assert_top_level_conformance, extract_stored_id};
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
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0.1.0"}
            }
        }),
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(resp["error"].is_null());

    // ALP-1761: server advertises 2025-06-18 (clean break, no
    // 2024-11-05 fallback). The dual-channel envelope and per-tool
    // outputSchema declarations are the load-bearing changes that
    // justify the version bump.
    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], "2025-06-18");
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

    let browse_tool = tools
        .iter()
        .find(|tool| tool["name"] == "cx_browse")
        .expect("cx_browse tool is advertised");
    let browse_props = browse_tool["inputSchema"]["properties"]
        .as_object()
        .expect("cx_browse inputSchema properties");
    for expected in [
        "scope",
        "scope_mode",
        "cwd",
        "include_resolution",
        "scope_path",
    ] {
        assert!(
            browse_props.contains_key(expected),
            "cx_browse inputSchema missing {expected}"
        );
    }
    assert_eq!(
        browse_props["scope_mode"]["enum"],
        json!(["resolved"]),
        "scope_mode should reserve only the implemented first-pass mode"
    );
    assert!(
        browse_tool["outputSchema"]["properties"]["resolution"].is_object(),
        "cx_browse outputSchema must document optional resolution metadata"
    );

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

    // ALP-1760: read tools carry a parallel `structuredContent`
    // projection matching the declared `outputSchema` from ALP-1759.
    // For cx_stats the shape is WebStatsView — flat counters plus
    // kinds/top_tags/scope_tree collections. Every field must be
    // present even on an empty store (kinds/top_tags/scope_tree are
    // possibly-empty collections, never absent keys).
    let structured = &result["structuredContent"];
    assert!(
        structured.is_object(),
        "cx_stats response must carry structuredContent"
    );
    for key in &[
        "active",
        "superseded",
        "scopes",
        "relations",
        "db_size_bytes",
        "kinds",
        "top_tags",
        "scope_tree",
    ] {
        assert!(
            structured.get(key).is_some(),
            "cx_stats structuredContent missing {key}"
        );
    }
    // Raw integer in structured channel. The YAML text channel
    // humanises db_size_bytes to a "4.2 MB" string; the structured
    // channel deliberately carries the u64 for type-checked clients.
    assert!(structured["db_size_bytes"].is_u64());

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
    // and renders one entry row per hit. After ALP-1767 phase 2 the
    // row format is `  - <title>` with no short-id column, so the
    // roundtrip check substring-matches on the unique title.
    let recall_text = recall_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(recall_text.contains("routing: search"));
    assert!(recall_text.contains("Protocol test fact"));

    // ALP-1760: recall also emits `structuredContent` shaped as
    // WebRecallView (header + entries + advisories). `cx_store`
    // deliberately skips this channel — write tools are text-only —
    // so the store-half above is asserted on `content` alone.
    let recall_structured = &recall_resp["result"]["structuredContent"];
    assert!(
        recall_structured.is_object(),
        "cx_recall response must carry structuredContent"
    );
    let recall_header = &recall_structured["header"];
    assert!(recall_header.is_object(), "recall header must be an object");
    assert_eq!(recall_header["routing"], "search");
    assert!(recall_header["query"].is_string());
    let recall_entries = recall_structured["entries"]
        .as_array()
        .expect("recall entries must be an array");
    assert!(
        !recall_entries.is_empty(),
        "recall must return at least one entry for a matching query"
    );
    // Every row carries the full UUID and the short-id prefix. The
    // roundtrip matches the stored UUID against the first row rather
    // than substring-searching because the structured channel is
    // field-keyed, not free text.
    let first_row = &recall_entries[0];
    assert_eq!(first_row["id"], stored_id);
    assert_eq!(first_row["title"], "Protocol test fact");
    assert!(recall_structured["advisories"].is_array());

    shutdown(child, stdin);
}

// ── Test 6: tools/call cx_export dual-channel shape ────────────────

/// Assert that `cx_export` emits an empty `content` array and a
/// populated `structuredContent` payload. `cx_export` is the only
/// tool whose canonical form is JSON (backup/restore fidelity beats
/// wire compactness), so the envelope builder emits `content: []`
/// and surfaces the JSON via `structuredContent` instead. Reading
/// `content[0]["type"]` on this response would panic on an empty
/// array — the assertions use `content.as_array().is_empty()` and
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

// ── Test 7: structuredContent conforms to declared outputSchema ────

/// Snapshot every read tool's `structuredContent` against its declared
/// `outputSchema`, loaded live from the same `tools/list` response a
/// real MCP client would consume. This is the load-bearing test for
/// ALP-1761: it locks in that ALP-1759 (outputSchema declarations) and
/// ALP-1760 (dual-channel envelope) stay coherent. If a worker bumps
/// the projection struct without updating tools.toml — or vice versa
/// — this test fails with a precise tool/key/type diagnostic.
///
/// Coverage: cx_recall, cx_browse, cx_get, cx_stats. cx_export is
/// covered by `protocol_tools_call_cx_export` above; its envelope
/// shape (empty content array, structured-only payload) is unique
/// enough to deserve a dedicated test rather than a generic loop.
#[test]
fn protocol_structuredcontent_conforms_to_outputschema() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    // Build a {tool_name -> outputSchema} map from the live tools/list
    // response. Loading from the wire (rather than re-parsing tools.toml
    // or importing the generated schema constant) is deliberate: this
    // test exists to catch drift between what the server *advertises*
    // and what it *emits*, so the schema source must be the wire shape.
    let list_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    );
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array");
    let schema_map: HashMap<String, Value> = tools
        .iter()
        .filter_map(|t| {
            let name = t["name"].as_str()?.to_owned();
            let schema = t.get("outputSchema")?.clone();
            Some((name, schema))
        })
        .collect();

    // Seed one entry so cx_recall returns a populated `entries` array
    // and cx_get has a real id to fetch. cx_browse and cx_stats both
    // tolerate empty stores, but exercising the populated path catches
    // shape regressions where empty arrays mask field-level breakage.
    let store_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "cx_store",
                "arguments": {
                    "title": "Schema conformance fact",
                    "body": "Seed row so cx_recall and cx_get exercise the populated path.",
                    "kind": "fact"
                }
            }
        }),
    );
    let store_text = store_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("cx_store ack carries text channel");
    let stored_id = extract_stored_id(store_text);

    // Drive every read tool through tools/call and validate its
    // structuredContent against the schema loaded from tools/list.
    // The `id` field starts at 10 and increments per call so a flaky
    // failure points at exactly which tool tripped the assertion.
    let cases: [(&str, Value); 4] = [
        ("cx_recall", json!({"query": "schema conformance"})),
        ("cx_browse", json!({})),
        ("cx_get", json!({"ids": [stored_id]})),
        ("cx_stats", json!({})),
    ];
    for (i, (tool, args)) in cases.iter().enumerate() {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &json!({
                "jsonrpc": "2.0",
                "id": 10 + i,
                "method": "tools/call",
                "params": {"name": tool, "arguments": args}
            }),
        );
        assert!(resp["error"].is_null(), "{tool}: tools/call failed: {resp}");

        // Dual-channel assertion: every read tool emits both
        // `content[0].text` (the YAML the LLM reads) and
        // `structuredContent` (the typed JSON for tooling). These two
        // channels are the load-bearing contract of ALP-1760 and the
        // reason ALP-1761 advertises 2025-06-18 in the first place.
        let content = resp["result"]["content"]
            .as_array()
            .unwrap_or_else(|| panic!("{tool}: result.content must be an array"));
        assert_eq!(
            content.len(),
            1,
            "{tool}: read tools emit exactly one text content block"
        );
        assert_eq!(content[0]["type"], "text", "{tool}: content[0].type");
        assert!(
            content[0]["text"].is_string(),
            "{tool}: content[0].text must be a string"
        );

        let structured = &resp["result"]["structuredContent"];
        assert!(
            structured.is_object(),
            "{tool}: result.structuredContent must be an object, got {structured}"
        );
        let schema = schema_map
            .get(*tool)
            .unwrap_or_else(|| panic!("{tool}: tools/list did not advertise an outputSchema"));
        assert_top_level_conformance(tool, schema, structured);
    }

    shutdown(child, stdin);
}
