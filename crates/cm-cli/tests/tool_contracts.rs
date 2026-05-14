use cm_cli::tool_contracts::{
    ParamShape, ScopeFragment, ScopeInputShape, ScopePolicy, contract_registry,
};
use serde_json::json;

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
fn broad_scope_schema_prefers_descendants_and_keeps_subtree_alias() {
    let registry = contract_registry();
    let search = registry
        .get("cx_search")
        .expect("cx_search contract exists");
    let scope = search.param("scope").expect("scope param exists");
    let schema = scope.input_schema_object();
    let variants = schema["oneOf"]
        .as_array()
        .expect("scope has oneOf variants");

    assert_eq!(
        variants[6]["properties"]["kind"]["enum"],
        json!(["descendants", "subtree"])
    );
}
