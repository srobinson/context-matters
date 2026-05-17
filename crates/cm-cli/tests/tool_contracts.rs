mod common;

use cm_capabilities::projection::{
    DepositReceipt, ForgetReceipt, ForgetReceiptError, StoreReceipt, UpdateReceipt,
};
use cm_cli::tool_contracts::{
    ParamShape, ScopeFragment, ScopeInputShape, ScopePolicy, contract_registry,
};
use cm_cli::tool_docs::{
    render_generated_instructions_rs, render_readme_md, render_server_instructions, render_skill_md,
};
use common::assert_top_level_conformance;
use serde_json::json;
use std::{fs, path::PathBuf};

#[test]
fn registry_models_read_tool_contract() {
    let registry = contract_registry();
    let recall = registry
        .get("cx_recall")
        .expect("cx_recall contract exists");

    assert_eq!(recall.name, "cx_recall");
    assert_eq!(
        recall.descriptions.short,
        "Priority context for one known scope"
    );
    assert_eq!(recall.scope_policy, ScopePolicy::SingularRead);
    assert_eq!(recall.output.type_name, "WebRecallView");
    assert!(recall.output.schema.is_some());
    assert_eq!(recall.cli.name, "recall");
    assert_eq!(recall.artifacts.mcp_schema_file, "cx_recall.json");
    assert_eq!(recall.required_param_names(), Vec::<&str>::new());
    assert_eq!(
        recall.examples[0].invocation,
        r#"cx_recall(query: "auth decisions", scope: {"kind":"path","path":"global/project:helioy"})"#
    );

    let scope = recall.param("scope").expect("scope param exists");
    assert!(!scope.required);
    assert_eq!(&scope.shape, &ParamShape::Scope(ScopeInputShape::Singular));
}

#[test]
fn registry_models_write_tool_contract() {
    let registry = contract_registry();
    let store = registry.get("cx_store").expect("cx_store contract exists");

    assert_eq!(store.name, "cx_store");
    assert_eq!(store.scope_policy, ScopePolicy::SingularWrite);
    assert_eq!(store.output.type_name, "StoreReceipt");
    assert_eq!(store.cli.name, "store");
    assert_eq!(store.artifacts.cli_help_prefix, "STORE");
    assert_eq!(store.required_param_names(), ["title", "body", "kind"]);

    let title = store.param("title").expect("title param exists");
    assert!(title.required);
    assert!(matches!(title.shape, ParamShape::Scalar(ref ty) if ty == "string"));

    let scope = store.param("scope").expect("scope param exists");
    assert_eq!(&scope.shape, &ParamShape::Scope(ScopeInputShape::Singular));

    let update = registry
        .get("cx_update")
        .expect("cx_update contract exists");
    assert_eq!(update.output.type_name, "UpdateReceipt");
}

#[test]
fn write_receipt_structs_match_contract_output_schemas() {
    let registry = contract_registry();
    let receipts = [
        (
            "cx_store",
            serde_json::to_value(StoreReceipt {
                id: "01950000-0000-7000-8000-000000000001".to_owned(),
                scope_path: "global".to_owned(),
                kind: "fact".to_owned(),
                content_hash: "a".repeat(64),
                superseded_id: None,
                scope_created: false,
            })
            .expect("store receipt serializes"),
        ),
        (
            "cx_deposit",
            serde_json::to_value(DepositReceipt {
                deposited: 1,
                entry_ids: vec!["01950000-0000-7000-8000-000000000002".to_owned()],
                summary_id: None,
                scope_path: "global".to_owned(),
            })
            .expect("deposit receipt serializes"),
        ),
        (
            "cx_update",
            serde_json::to_value(UpdateReceipt {
                id: "01950000-0000-7000-8000-000000000003".to_owned(),
                content_hash: "b".repeat(64),
            })
            .expect("update receipt serializes"),
        ),
        (
            "cx_forget",
            serde_json::to_value(ForgetReceipt {
                forgotten: 1,
                already_inactive: 2,
                not_found: 3,
                errors: vec![ForgetReceiptError {
                    id: "01950000-0000-7000-8000-000000000004".to_owned(),
                    error: "storage error".to_owned(),
                }],
            })
            .expect("forget receipt serializes"),
        ),
    ];

    for (tool_name, value) in receipts {
        let schema = registry
            .get(tool_name)
            .and_then(|tool| tool.output.schema.as_ref())
            .unwrap_or_else(|| panic!("{tool_name} output schema exists"));
        assert_top_level_conformance(tool_name, schema, &value);
    }
}

#[test]
fn registry_models_scope_params_as_shared_fragments() {
    let registry = contract_registry();
    let migrated = [
        ("cx_recall", ScopeInputShape::Singular),
        ("cx_search", ScopeInputShape::Broad),
        ("cx_store", ScopeInputShape::Singular),
        ("cx_deposit", ScopeInputShape::Singular),
        ("cx_browse", ScopeInputShape::Broad),
        ("cx_export", ScopeInputShape::Broad),
    ];

    for (tool_name, expected_shape) in migrated {
        let tool = registry.get(tool_name).expect("migrated tool exists");
        let scope = tool.param("scope").expect("migrated tool has scope param");

        assert_eq!(
            &scope.shape,
            &ParamShape::Scope(expected_shape),
            "{tool_name} scope param must use a shared scope fragment"
        );
    }

    assert_eq!(
        ScopeInputShape::Singular.fragments(),
        &[
            ScopeFragment::ExactPathString,
            ScopeFragment::Path,
            ScopeFragment::CwdInferred,
            ScopeFragment::Project,
            ScopeFragment::Repo,
            ScopeFragment::Session,
        ]
    );
    assert_eq!(
        ScopeInputShape::Broad.fragments(),
        &[
            ScopeFragment::ExactPathString,
            ScopeFragment::Path,
            ScopeFragment::CwdInferred,
            ScopeFragment::Project,
            ScopeFragment::Repo,
            ScopeFragment::Session,
            ScopeFragment::Descendants,
            ScopeFragment::Set,
            ScopeFragment::All,
        ]
    );
}

#[test]
fn broad_scope_schema_emits_flat_codex_safe_shape() {
    let registry = contract_registry();
    let search = registry
        .get("cx_search")
        .expect("cx_search contract exists");
    let scope = search.param("scope").expect("scope param exists");
    let schema = scope.input_schema_object();

    // No top-level combinators (ALP-2476). The schema is a concrete
    // object so Codex's strict-mode validator and Gemini accept it.
    assert!(
        schema.get("oneOf").is_none()
            && schema.get("anyOf").is_none()
            && schema.get("allOf").is_none(),
        "broad scope schema must not use top-level combinators"
    );
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(schema["required"], json!(["kind"]));

    // Canonical kinds only. `descendants` is a parser-level alias kept
    // for backward compatibility; the schema advertises `subtree`.
    let kinds = schema["properties"]["kind"]["enum"]
        .as_array()
        .expect("kind has enum");
    let kind_strings: Vec<&str> = kinds.iter().filter_map(|v| v.as_str()).collect();
    assert!(kind_strings.contains(&"subtree"));
    assert!(kind_strings.contains(&"set"));
    assert!(kind_strings.contains(&"all"));
    assert!(kind_strings.contains(&"path"));
    assert!(!kind_strings.contains(&"descendants"));

    // Broad shape includes `paths` (for set); singular omits it.
    assert!(schema["properties"].get("paths").is_some());

    // Canonical examples are emitted for the agent.
    let examples = schema["examples"].as_array().expect("examples present");
    assert!(examples.iter().any(|e| e["kind"] == json!("all")));
    assert!(examples.iter().any(|e| e["kind"] == json!("subtree")));
}

#[test]
fn singular_scope_schema_omits_set_and_paths() {
    let registry = contract_registry();
    let recall = registry
        .get("cx_recall")
        .expect("cx_recall contract exists");
    let scope = recall.param("scope").expect("scope param exists");
    let schema = scope.input_schema_object();

    assert_eq!(schema["type"], json!("object"));
    assert!(schema["properties"].get("paths").is_none());

    let kinds = schema["properties"]["kind"]["enum"]
        .as_array()
        .expect("kind has enum");
    let kind_strings: Vec<&str> = kinds.iter().filter_map(|v| v.as_str()).collect();
    assert!(!kind_strings.contains(&"set"));
    assert!(!kind_strings.contains(&"all"));
    assert!(!kind_strings.contains(&"subtree"));
    assert!(kind_strings.contains(&"path"));
    assert!(kind_strings.contains(&"cwd_inferred"));
}

#[test]
fn generated_contract_docs_are_fresh() {
    let registry = contract_registry();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("cm-cli lives under crates")
        .parent()
        .expect("crates lives under workspace");

    let instructions = render_server_instructions(registry.tools());
    let expected = [
        (
            manifest.join("templates/SKILL.md"),
            render_skill_md(registry.skill(), registry.tools()),
        ),
        (
            manifest.join("src/mcp/generated_instructions.rs"),
            render_generated_instructions_rs(&instructions),
        ),
        (
            workspace.join("README.md"),
            render_readme_md(registry.tools()),
        ),
    ];

    for (path, expected_content) in expected {
        let actual = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        assert_eq!(
            actual,
            expected_content,
            "{} is stale against the contract registry",
            path.display()
        );
    }
}
