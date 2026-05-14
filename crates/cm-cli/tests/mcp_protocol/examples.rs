use crate::common::{call_tool, send_request, shutdown, spawn_server};
use cm_capabilities::scope::ScopeSelector;
use cm_cli::tool_contracts::{ParamShape, ScopeFragment, ScopeInputShape, contract_registry};
use serde_json::{Value, json};

#[test]
fn contract_tool_examples_are_mcp_argument_objects() {
    let registry = contract_registry();

    for tool in registry.tools() {
        for example in &tool.examples {
            let args = example
                .arguments
                .as_object()
                .unwrap_or_else(|| panic!("{} example is not an argument object", tool.name));

            for required in tool.required_param_names() {
                assert!(
                    args.contains_key(required),
                    "{} example `{}` is missing required argument `{required}`",
                    tool.name,
                    example.invocation
                );
            }

            for (arg_name, value) in args {
                let param = tool.param(arg_name).unwrap_or_else(|| {
                    panic!(
                        "{} example `{}` contains unknown argument `{arg_name}`",
                        tool.name, example.invocation
                    )
                });

                if matches!(param.shape, ParamShape::Scope(_)) {
                    serde_json::from_value::<ScopeSelector>(value.clone()).unwrap_or_else(|err| {
                        panic!(
                            "{} example `{}` has invalid scope argument: {err}",
                            tool.name, example.invocation
                        )
                    });
                }
            }
        }
    }
}

#[test]
fn documented_scope_request_examples_parse_as_scope_selectors() {
    for fragment in documented_scope_fragments() {
        let args = scope_request_args(fragment);
        let scope = args
            .get("scope")
            .unwrap_or_else(|| panic!("{fragment:?} request example has no scope field: {args}"));

        serde_json::from_value::<ScopeSelector>(scope.clone()).unwrap_or_else(|err| {
            panic!(
                "{fragment:?} request example does not parse as an MCP scope selector: {err}; example: {}",
                fragment.request_example()
            )
        });
    }
}

#[test]
fn documented_scope_examples_execute_representative_mcp_calls() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    let mut write_args = scope_request_args(ScopeFragment::ExactPathString);
    write_args["title"] = json!("Documented scoped write example");
    write_args["body"] = json!("Exercises the documented canonical path selector.");
    write_args["kind"] = json!("decision");

    let store_resp = send_request(
        &mut stdin,
        &mut stdout,
        &call_tool(write_args, "cx_store", 2),
    );
    assert!(
        store_resp["error"].is_null(),
        "documented scoped write example failed: {store_resp}"
    );
    assert_eq!(
        store_resp["result"]["structuredContent"]["scope_path"],
        "global/project:helioy/repo:context-matters"
    );

    let export_resp = send_request(
        &mut stdin,
        &mut stdout,
        &call_tool(
            scope_request_args(ScopeFragment::Descendants),
            "cx_export",
            3,
        ),
    );
    assert!(
        export_resp["error"].is_null(),
        "documented broad export example failed: {export_resp}"
    );
    assert_eq!(export_resp["result"]["structuredContent"]["count"], 1);

    shutdown(child, stdin);
}

#[test]
fn self_contained_contract_examples_execute_through_mcp() {
    let dir = tempfile::tempdir().unwrap();
    let (child, mut stdin, mut stdout) = spawn_server(&dir);

    send_request(
        &mut stdin,
        &mut stdout,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    for (offset, tool_name) in [
        "cx_store",
        "cx_deposit",
        "cx_recall",
        "cx_search",
        "cx_browse",
        "cx_stats",
        "cx_export",
    ]
    .into_iter()
    .enumerate()
    {
        let example = &contract_registry()
            .get(tool_name)
            .unwrap_or_else(|| panic!("{tool_name} contract exists"))
            .examples[0];
        let resp = send_request(
            &mut stdin,
            &mut stdout,
            &call_tool(example.arguments.clone(), tool_name, 10 + offset as u64),
        );
        assert!(
            resp["error"].is_null(),
            "{tool_name} documented example `{}` failed: {resp}",
            example.invocation
        );
    }

    shutdown(child, stdin);
}

fn documented_scope_fragments() -> Vec<ScopeFragment> {
    let mut fragments = ScopeInputShape::Singular.fragments().to_vec();
    fragments.extend(
        ScopeInputShape::Broad
            .fragments()
            .iter()
            .copied()
            .filter(|fragment| !ScopeInputShape::Singular.fragments().contains(fragment)),
    );
    fragments
}

fn scope_request_args(fragment: ScopeFragment) -> Value {
    serde_json::from_str(fragment.request_example()).unwrap_or_else(|err| {
        panic!(
            "{fragment:?} request example is not valid JSON: {err}; example: {}",
            fragment.request_example()
        )
    })
}
