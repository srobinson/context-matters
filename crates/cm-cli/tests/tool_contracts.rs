use cm_cli::tool_contracts::{ParamShape, ScopePolicy, contract_registry};

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
    assert!(matches!(scope.shape, ParamShape::CustomSchema(_)));
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
    assert!(matches!(scope.shape, ParamShape::CustomSchema(_)));
}
