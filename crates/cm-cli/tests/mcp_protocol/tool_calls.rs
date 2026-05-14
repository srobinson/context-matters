use crate::common::{
    call_tool, extract_stored_id, send_request, shutdown, spawn_server, tool_error_message,
};
use serde_json::{Value, json};

fn structured_entry_titles(resp: &Value) -> Vec<String> {
    resp["result"]["structuredContent"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("search response entries missing: {resp}"))
        .iter()
        .map(|entry| {
            entry["title"]
                .as_str()
                .unwrap_or_else(|| panic!("search entry missing title: {entry}"))
                .to_owned()
        })
        .collect()
}

#[test]
fn protocol_tools_call_cx_stats() {
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
    // For cx_stats the shape is WebStatsView: flat counters plus
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

#[test]
fn protocol_migrated_scope_tools_reject_scope_path() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let cases = [
        (
            "cx_browse",
            json!({"scope_path": "global"}),
            "use 'scope' instead of 'scope_path'",
        ),
        (
            "cx_recall",
            json!({"scope_path": "global"}),
            "use 'scope' instead of 'scope_path'",
        ),
        (
            "cx_store",
            json!({
                "title": "Bad",
                "body": "Body.",
                "kind": "fact",
                "scope_path": "global"
            }),
            "use 'scope' instead of 'scope_path'",
        ),
        (
            "cx_deposit",
            json!({
                "exchanges": [{"user": "u", "assistant": "a"}],
                "scope_path": "global"
            }),
            "use 'scope' instead of 'scope_path'",
        ),
        (
            "cx_export",
            json!({"scope_path": "global"}),
            "use 'scope' instead of 'scope_path'",
        ),
    ];

    for (index, (tool, args, expected)) in cases.into_iter().enumerate() {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(args, tool, 10 + index as u64),
        );
        let message = tool_error_message(&resp);
        assert!(
            message.contains(expected),
            "{tool} error should contain {expected:?}, got {message:?}"
        );
    }

    shutdown(child, stdin);
}

#[test]
fn protocol_non_scope_tools_reject_removed_scope_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let cases = [
        ("cx_get", json!({"ids": [], "scope_path": "global"})),
        (
            "cx_update",
            json!({
                "id": "018f0000-0000-7000-8000-000000000000",
                "title": "Bad",
                "scope_path": "global"
            }),
        ),
        ("cx_forget", json!({"ids": [], "scope_mode": "resolved"})),
        ("cx_stats", json!({"scope_mode": "resolved"})),
    ];

    for (index, (tool, args)) in cases.into_iter().enumerate() {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(args, tool, 40 + index as u64),
        );
        let message = tool_error_message(&resp);
        assert!(
            message.contains("use 'scope' instead of"),
            "{tool} should redirect legacy scope inputs, got {message:?}"
        );
    }

    shutdown(child, stdin);
}

#[test]
fn protocol_stats_rejects_unknown_fields() {
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
        &call_tool(json!({"bogus_field": "x"}), "cx_stats", 2),
    );
    let message = tool_error_message(&resp);
    assert!(
        message.contains("unknown field `bogus_field`"),
        "cx_stats should reject unknown field, got {message:?}"
    );

    shutdown(child, stdin);
}

#[test]
fn protocol_write_scope_tools_accept_legacy_exact_scope() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let store_resp = send_request(
        &mut stdin,
        &mut stdout,
        &call_tool(
            json!({
                "title": "Exact scope fact",
                "body": "Body.",
                "kind": "fact",
                "scope": "global/project:helioy"
            }),
            "cx_store",
            2,
        ),
    );
    assert!(store_resp["error"].is_null(), "store failed: {store_resp}");
    let store_text = store_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(store_text.contains("scope: global/project:helioy"));

    let export_resp = send_request(
        &mut stdin,
        &mut stdout,
        &call_tool(json!({"scope": "global/project:helioy"}), "cx_export", 5),
    );
    assert!(
        export_resp["error"].is_null(),
        "export failed: {export_resp}"
    );

    shutdown(child, stdin);
}

#[test]
fn protocol_search_accepts_structured_scope_variants_and_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    for (index, (title, scope)) in [
        ("Global search fact", "global"),
        ("Project search fact", "global/project:helioy"),
        (
            "Repo search fact",
            "global/project:helioy/repo:context-matters",
        ),
        ("Other search fact", "global/project:attention-matters"),
    ]
    .into_iter()
    .enumerate()
    {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(
                json!({
                    "title": title,
                    "body": format!("searchneedle body for {title}"),
                    "kind": "fact",
                    "scope": scope
                }),
                "cx_store",
                10 + index as u64,
            ),
        );
        assert!(resp["error"].is_null(), "store failed: {resp}");
    }

    let cases = [
        (
            json!({"kind": "path", "path": "global/project:helioy"}),
            vec!["Project search fact"],
        ),
        (
            json!({"kind": "cwd_inferred", "cwd": dir.path()}),
            vec!["Global search fact"],
        ),
        (
            json!({"kind": "subtree", "path": "global/project:helioy"}),
            vec!["Project search fact", "Repo search fact"],
        ),
        (
            json!({
                "kind": "set",
                "paths": ["global", "global/project:attention-matters"]
            }),
            vec!["Global search fact", "Other search fact"],
        ),
        (
            json!({"kind": "all"}),
            vec![
                "Global search fact",
                "Project search fact",
                "Repo search fact",
                "Other search fact",
            ],
        ),
    ];

    for (index, (scope, expected_titles)) in cases.into_iter().enumerate() {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(
                json!({"query": "searchneedle", "scope": scope, "limit": 20}),
                "cx_search",
                30 + index as u64,
            ),
        );
        assert!(resp["error"].is_null(), "search failed: {resp}");
        let titles = structured_entry_titles(&resp);
        for expected in expected_titles {
            assert!(
                titles.iter().any(|title| title == expected),
                "search titles {titles:?} missing {expected}"
            );
        }
    }

    let first_page = send_request(
        &mut stdin,
        &mut stdout,
        &call_tool(
            json!({"query": "searchneedle", "scope": {"kind": "all"}, "limit": 1}),
            "cx_search",
            40,
        ),
    );
    let cursor = first_page["result"]["structuredContent"]["header"]["next_cursor"]
        .as_str()
        .unwrap_or_else(|| panic!("first search page missing next_cursor: {first_page}"));
    let second_page = send_request(
        &mut stdin,
        &mut stdout,
        &call_tool(
            json!({
                "query": "searchneedle",
                "scope": {"kind": "all"},
                "limit": 1,
                "cursor": cursor
            }),
            "cx_search",
            41,
        ),
    );
    assert!(
        second_page["error"].is_null(),
        "cursor search failed: {second_page}"
    );
    assert_eq!(structured_entry_titles(&second_page).len(), 1);

    shutdown(child, stdin);
}

#[test]
fn protocol_search_empty_query_returns_capability_error() {
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
        &call_tool(
            json!({"query": "   ", "scope": {"kind": "all"}}),
            "cx_search",
            2,
        ),
    );
    let message = tool_error_message(&resp);
    assert!(
        message.contains("query is required; use cx_browse"),
        "empty search query should use shared capability error, got {message:?}"
    );

    shutdown(child, stdin);
}

#[test]
fn protocol_migrated_scope_tools_reject_auto_scope() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let cases = [
        ("cx_browse", json!({"scope": "auto"})),
        ("cx_recall", json!({"scope": "auto"})),
        (
            "cx_store",
            json!({"title": "Bad", "body": "Body.", "kind": "fact", "scope": "auto"}),
        ),
        (
            "cx_deposit",
            json!({"exchanges": [{"user": "u", "assistant": "a"}], "scope": "auto"}),
        ),
        ("cx_export", json!({"scope": "auto"})),
    ];

    for (index, (tool, args)) in cases.into_iter().enumerate() {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(args, tool, 20 + index as u64),
        );
        let message = tool_error_message(&resp);
        assert!(
            message.contains("instead of scope='auto'"),
            "{tool} error should reject auto, got {message:?}"
        );
    }

    shutdown(child, stdin);
}

#[test]
fn protocol_browse_rejects_scope_mode_input() {
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
        &call_tool(json!({"scope_mode": "resolved"}), "cx_browse", 2),
    );
    let message = tool_error_message(&resp);
    assert!(
        message.contains("use 'scope' instead of 'scope_mode'"),
        "unexpected error: {message}"
    );

    shutdown(child, stdin);
}

#[test]
fn protocol_migrated_scope_tools_reject_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let cases = [
        ("cx_browse", json!({"bogus_field": "x"})),
        ("cx_recall", json!({"bogus_field": "x"})),
        (
            "cx_store",
            json!({
                "title": "Bad",
                "body": "Body.",
                "kind": "fact",
                "bogus_field": "x"
            }),
        ),
        (
            "cx_deposit",
            json!({
                "exchanges": [{"user": "u", "assistant": "a"}],
                "bogus_field": "x"
            }),
        ),
        ("cx_export", json!({"bogus_field": "x"})),
    ];

    for (index, (tool, args)) in cases.into_iter().enumerate() {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(args, tool, 30 + index as u64),
        );
        let message = tool_error_message(&resp);
        assert!(
            message.contains("unknown field `bogus_field`"),
            "{tool} should reject unknown field, got {message:?}"
        );
    }

    shutdown(child, stdin);
}

#[test]
fn protocol_store_and_recall_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

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
    // WebRecallView (header + entries + advisories).
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
