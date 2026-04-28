use crate::common::{send_request, shutdown, spawn_server};
use serde_json::json;

#[test]
fn protocol_tools_list() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

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
    for expected in ["scope", "cwd", "include_resolution"] {
        assert!(
            browse_props.contains_key(expected),
            "cx_browse inputSchema missing {expected}"
        );
    }
    for removed in ["scope_path", "scope_mode"] {
        assert!(
            !browse_props.contains_key(removed),
            "cx_browse inputSchema still exposes removed input {removed}"
        );
    }
    assert!(
        browse_tool["outputSchema"]["properties"]["resolution"].is_object(),
        "cx_browse outputSchema must document optional resolution metadata"
    );

    for migrated in [
        "cx_browse",
        "cx_recall",
        "cx_store",
        "cx_deposit",
        "cx_export",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == migrated)
            .unwrap_or_else(|| panic!("{migrated} tool is advertised"));
        let props = tool["inputSchema"]["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{migrated} inputSchema properties"));
        assert!(
            props.contains_key("scope"),
            "{migrated} inputSchema must expose scope"
        );
        for removed in ["scope_path", "scope_mode"] {
            assert!(
                !props.contains_key(removed),
                "{migrated} inputSchema still exposes removed input {removed}"
            );
        }
        assert!(
            !tool["description"]
                .as_str()
                .expect("tool description")
                .contains("scope_path"),
            "{migrated} description should not mention public scope_path input"
        );
    }

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
