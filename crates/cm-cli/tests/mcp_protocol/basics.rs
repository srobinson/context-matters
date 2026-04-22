use crate::common::{send_request, shutdown, spawn_server};
use serde_json::json;

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

#[test]
fn protocol_unknown_method() {
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
