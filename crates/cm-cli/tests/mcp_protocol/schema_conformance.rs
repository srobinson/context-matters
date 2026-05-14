use std::collections::HashMap;

use crate::common::{
    assert_top_level_conformance, extract_stored_id, send_request, shutdown, spawn_server,
};
use serde_json::{Value, json};

/// Snapshot every dual-channel tool's `structuredContent` against its declared
/// `outputSchema`, loaded live from the same `tools/list` response a
/// real MCP client would consume. This is the load-bearing test for
/// ALP-1761: it locks in that ALP-1759 (outputSchema declarations) and
/// ALP-1760 (dual-channel envelope) stay coherent. If a worker bumps
/// the projection struct without updating tools.toml, or changes
/// tools.toml without updating the projection struct, this test fails
/// with a precise tool/key/type diagnostic.
///
/// Coverage includes all read projections plus the cx_store, cx_update,
/// cx_deposit, and cx_forget write receipts. cx_export is covered by
/// `protocol_tools_call_cx_export` above; its envelope shape (empty
/// content array, structured-only payload) is unique enough to deserve
/// a dedicated test rather than a generic loop.
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

    // Seed one entry so cx_recall and cx_search return populated
    // `entries` arrays and cx_get has a real id to fetch. cx_browse and cx_stats both
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
                    "body": "Seed row so cx_recall, cx_search, and cx_get exercise the populated path.",
                    "kind": "fact"
                }
            }
        }),
    );
    let store_text = store_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("cx_store ack carries text channel");
    let stored_id = extract_stored_id(store_text);
    assert_top_level_conformance(
        "cx_store",
        schema_map
            .get("cx_store")
            .expect("cx_store outputSchema advertised"),
        &store_resp["result"]["structuredContent"],
    );

    let update_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "cx_update",
                "arguments": {
                    "id": &stored_id,
                    "title": "Schema conformance updated"
                }
            }
        }),
    );
    assert!(
        update_resp["error"].is_null(),
        "cx_update failed: {update_resp}"
    );
    assert_top_level_conformance(
        "cx_update",
        schema_map
            .get("cx_update")
            .expect("cx_update outputSchema advertised"),
        &update_resp["result"]["structuredContent"],
    );

    let deposit_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "cx_deposit",
                "arguments": {
                    "exchanges": [{"user": "u", "assistant": "a"}]
                }
            }
        }),
    );
    assert!(
        deposit_resp["error"].is_null(),
        "cx_deposit failed: {deposit_resp}"
    );
    assert_top_level_conformance(
        "cx_deposit",
        schema_map
            .get("cx_deposit")
            .expect("cx_deposit outputSchema advertised"),
        &deposit_resp["result"]["structuredContent"],
    );

    // Drive every read tool through tools/call and validate its
    // structuredContent against the schema loaded from tools/list.
    // The `id` field starts at 10 and increments per call so a flaky
    // failure points at exactly which tool tripped the assertion.
    let cases: [(&str, Value); 5] = [
        ("cx_recall", json!({"query": "schema conformance"})),
        (
            "cx_search",
            json!({"query": "schema conformance", "scope": {"kind": "all"}}),
        ),
        ("cx_browse", json!({})),
        ("cx_get", json!({"ids": [&stored_id]})),
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

    let forget_resp = send_request(
        &mut stdin,
        &mut stdout,
        &json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "cx_forget",
                "arguments": {
                    "ids": [&stored_id]
                }
            }
        }),
    );
    assert!(
        forget_resp["error"].is_null(),
        "cx_forget failed: {forget_resp}"
    );
    assert_top_level_conformance(
        "cx_forget",
        schema_map
            .get("cx_forget")
            .expect("cx_forget outputSchema advertised"),
        &forget_resp["result"]["structuredContent"],
    );

    shutdown(child, stdin);
}
