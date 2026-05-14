use crate::common::{send_request, shutdown, spawn_server};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

const MIGRATED_SCOPE_TOOLS: [&str; 6] = [
    "cx_browse",
    "cx_recall",
    "cx_search",
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
    assert_eq!(tools.len(), 10, "expected 10 MCP tools");

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    for expected in &[
        "cx_recall",
        "cx_search",
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
    for expected in ["scope", "include_resolution"] {
        assert!(
            browse_props.contains_key(expected),
            "cx_browse inputSchema missing {expected}"
        );
    }
    for removed in ["scope_path", "scope_mode", "cwd"] {
        assert!(
            !browse_props.contains_key(removed),
            "cx_browse inputSchema still exposes removed input {removed}"
        );
    }
    assert!(
        browse_tool["outputSchema"]["properties"]["resolution"].is_object(),
        "cx_browse outputSchema must document optional resolution metadata"
    );

    let search_tool = tools
        .iter()
        .find(|tool| tool["name"] == "cx_search")
        .expect("cx_search tool is advertised");
    let search_description = search_tool["description"]
        .as_str()
        .expect("cx_search description is text");
    for expected in ["FTS5 BM25-ranked", "Use cx_search", "Use cx_recall"] {
        assert!(
            search_description.contains(expected),
            "cx_search description should disambiguate recall/search with {expected:?}"
        );
    }
    let search_props = search_tool["inputSchema"]["properties"]
        .as_object()
        .expect("cx_search inputSchema properties");
    assert_eq!(
        search_tool["inputSchema"]["required"],
        json!(["query", "scope"]),
        "cx_search must require query and structured scope"
    );
    assert!(
        search_props["scope"]["oneOf"].is_array(),
        "cx_search scope input must model ScopeInput variants"
    );
    for read_tool_name in ["cx_recall", "cx_browse"] {
        let read_tool = tools
            .iter()
            .find(|tool| tool["name"] == read_tool_name)
            .unwrap_or_else(|| panic!("{read_tool_name} tool is advertised"));
        assert!(
            read_tool["inputSchema"]["properties"]["scope"]["oneOf"].is_array(),
            "{read_tool_name} scope input must model ScopeInput variants"
        );
    }
    assert!(
        search_tool["outputSchema"]["properties"]["header"]["properties"]["next_cursor"]
            .is_object(),
        "cx_search outputSchema must expose next_cursor for pagination"
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

    // Every tool that emits structuredContent advertises outputSchema so MCP
    // clients can validate both read projections and write receipts.
    let structured_tools = [
        "cx_recall",
        "cx_search",
        "cx_browse",
        "cx_get",
        "cx_stats",
        "cx_export",
        "cx_store",
        "cx_deposit",
        "cx_update",
        "cx_forget",
    ];
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let has_output_schema = tool.get("outputSchema").is_some_and(|v| v.is_object());
        if structured_tools.contains(&name) {
            assert!(has_output_schema, "tool {name} missing outputSchema");
        }
    }

    shutdown(child, stdin);
}

#[test]
fn public_scope_artifacts_do_not_expose_removed_request_terms() {
    assert_tools_toml_has_only_scope_request_params();
    assert_generated_scope_schema_inputs_are_current();

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = workspace_root();
    for (relative, path) in [
        (
            "src/cli/generated_help.rs",
            manifest.join("src/cli/generated_help.rs"),
        ),
        ("templates/SKILL.md", manifest.join("templates/SKILL.md")),
        (
            "src/mcp/generated_schema.rs",
            manifest.join("src/mcp/generated_schema.rs"),
        ),
        (
            "src/mcp/generated_instructions.rs",
            manifest.join("src/mcp/generated_instructions.rs"),
        ),
        ("README.md", workspace.join("README.md")),
    ] {
        let content =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {relative}: {e}"));
        assert_no_public_scope_stale_terms(relative, &content);
    }
    assert_skill_doc_explains_scope_request_boundary(&manifest);
    assert_skill_doc_explains_search_contract(&manifest);
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
        let scope = &props["scope"];
        let scope_description = scope["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{relative} scope input has description"));
        if matches!(
            tool,
            "cx_recall" | "cx_browse" | "cx_search" | "cx_store" | "cx_deposit" | "cx_export"
        ) {
            assert!(
                scope["oneOf"].is_array(),
                "{relative} scope input should model ScopeInput variants with oneOf"
            );
        }
        if matches!(tool, "cx_recall" | "cx_browse" | "cx_search") {
            assert!(
                scope_description.contains("cwd_inferred"),
                "{relative} scope input should describe cwd_inferred"
            );
        }
        if tool == "cx_browse" || tool == "cx_search" {
            for vocabulary in ["descendants", "set", "all"] {
                assert!(
                    scope_description.contains(vocabulary),
                    "{relative} scope input should describe {vocabulary} selectors"
                );
            }
        }
        assert!(
            !scope_description.contains("auto"),
            "{relative} scope input still describes auto inference"
        );
        if tool == "cx_browse" {
            assert!(
                !props.contains_key("cwd"),
                "{relative} inputSchema still exposes top-level cwd"
            );
        }
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
        "Public requests select scope through the `scope` field.",
        r#"cx_browse(scope: {"kind":"cwd_inferred","cwd":"/path/to/repo"})"#,
        "`cwd_inferred` resolves linked git worktrees to the source repository identity.",
        "Persisted entries and export rows include `scope_path`.",
    ] {
        assert!(
            content.contains(required),
            "generated skill doc missing scope migration boundary text: {required}"
        );
    }
}

fn assert_skill_doc_explains_search_contract(manifest: &Path) {
    let content = fs::read_to_string(manifest.join("templates/SKILL.md"))
        .expect("generated skill doc should be readable");
    for required in [
        "| `cx_search` | Content search across wide or unknown scopes",
        "Use cx_search when you have a query and want results from multiple scopes",
        "`cursor` | string | no | Opaque pagination cursor from a previous cx_search response",
    ] {
        assert!(
            content.contains(required),
            "generated skill doc missing cx_search contract text: {required}"
        );
    }
}
