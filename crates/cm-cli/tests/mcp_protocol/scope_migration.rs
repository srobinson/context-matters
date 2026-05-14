use crate::common::{call_tool, send_request, shutdown, spawn_server, tool_error_message};
use serde_json::json;

#[test]
fn protocol_read_scope_tools_accept_structured_exact_scope() {
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

    for (id, tool) in [(3, "cx_recall"), (4, "cx_browse")] {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(
                json!({"scope": {"kind": "path", "path": "global/project:helioy"}}),
                tool,
                id,
            ),
        );
        assert!(resp["error"].is_null(), "{tool} failed: {resp}");
    }

    shutdown(child, stdin);
}

#[test]
fn protocol_read_scope_tools_accept_plain_string_scope() {
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

    for (id, tool) in [(3, "cx_recall"), (4, "cx_browse")] {
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(json!({"scope": "global/project:helioy"}), tool, id),
        );
        assert!(
            resp["error"].is_null(),
            "{tool} should accept plain string scope, got {resp}"
        );
    }

    shutdown(child, stdin);
}

#[test]
fn protocol_browse_rejects_top_level_cwd() {
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
            json!({"cwd": "/tmp/helioy/context-matters"}),
            "cx_browse",
            2,
        ),
    );
    let message = tool_error_message(&resp);
    assert!(
        message.contains("unknown field `cwd`"),
        "cx_browse should reject top-level cwd, got {message:?}"
    );

    shutdown(child, stdin);
}
