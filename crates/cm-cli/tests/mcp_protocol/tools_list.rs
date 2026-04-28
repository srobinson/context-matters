use crate::common::{send_request, shutdown, spawn_server};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

const MIGRATED_SCOPE_TOOLS: [&str; 5] = [
    "cx_browse",
    "cx_recall",
    "cx_store",
    "cx_deposit",
    "cx_export",
];

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

    for migrated in MIGRATED_SCOPE_TOOLS {
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
        assert_eq!(
            tool["inputSchema"]["additionalProperties"],
            false,
            "{} inputSchema must reject stale or unknown public request fields",
            tool["name"].as_str().expect("tool name")
        );
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

#[test]
fn public_scope_artifacts_do_not_expose_removed_request_terms() {
    assert_tools_toml_has_only_scope_request_params();
    assert_generated_scope_schema_inputs_are_current();

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "src/cli/generated_help.rs",
        "templates/SKILL.md",
        "src/mcp/generated_schema.rs",
    ] {
        let content = fs::read_to_string(manifest.join(relative))
            .unwrap_or_else(|e| panic!("failed to read {relative}: {e}"));
        assert_no_public_scope_stale_terms(relative, &content);
    }
    assert_skill_doc_explains_scope_request_boundary(&manifest);
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cm-cli lives under crates/")
        .parent()
        .expect("crates/ lives under workspace root")
        .to_path_buf()
}

fn assert_tools_toml_has_only_scope_request_params() {
    let content = fs::read_to_string(workspace_root().join("tools.toml"))
        .expect("tools.toml should be readable");
    assert_no_public_scope_stale_terms("tools.toml", &content);

    for stale in [
        "name            = \"scope_path\"",
        "name            = \"scope_mode\"",
        "cli_flag        = \"--scope-path\"",
        "cli_flag        = \"--scope-mode\"",
        "scope=auto",
        "scope='auto'",
        "scope: \"auto\"",
    ] {
        assert!(
            !content.contains(stale),
            "tools.toml exposes removed public scope request term {stale}"
        );
    }
}

fn assert_generated_scope_schema_inputs_are_current() {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/mcp/generated_schema");
    for tool in MIGRATED_SCOPE_TOOLS {
        let relative = format!("src/mcp/generated_schema/{tool}.json");
        let content = fs::read_to_string(schema_dir.join(format!("{tool}.json")))
            .unwrap_or_else(|e| panic!("failed to read {relative}: {e}"));
        let schema: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("{relative} should be valid JSON: {e}"));
        let input = &schema["inputSchema"];
        assert_eq!(
            input["additionalProperties"], false,
            "{relative} inputSchema must reject stale or unknown public request fields"
        );
        let input_text = serde_json::to_string(input).expect("input schema serializes");
        for removed in ["scope_path", "scope_mode"] {
            assert!(
                !input_text.contains(removed),
                "{relative} inputSchema exposes removed request field {removed}"
            );
        }
        let props = input["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{relative} inputSchema has properties"));
        assert!(
            props.contains_key("scope"),
            "{relative} missing scope input"
        );
        let scope_description = props["scope"]["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{relative} scope input has description"));
        assert!(
            scope_description.contains("reserved value")
                && scope_description.contains("cwd_inferred"),
            "{relative} scope input should describe cwd_inferred as reserved value"
        );
        assert!(
            !scope_description.contains("auto"),
            "{relative} scope input still describes auto inference"
        );
    }
}

fn assert_no_public_scope_stale_terms(relative: &str, content: &str) {
    for stale in ["--scope-path", "--scope-mode", "scope=auto", "scope='auto'"] {
        assert!(
            !content.contains(stale),
            "{relative} contains stale public scope term {stale}"
        );
    }
}

fn assert_skill_doc_explains_scope_request_boundary(manifest: &Path) {
    let content = fs::read_to_string(manifest.join("templates/SKILL.md"))
        .expect("generated skill doc should be readable");
    for required in [
        "Public request inputs use `scope` only.",
        r#"cx_browse(scope: "cwd_inferred", cwd: "/path/to/repo")"#,
        "`cwd_inferred` is the reserved value for cwd based scope resolution.",
        "`scope_path` may still appear in persisted entries, export rows, and response data",
    ] {
        assert!(
            content.contains(required),
            "generated skill doc missing scope migration boundary text: {required}"
        );
    }
}
