//! CLI and MCP adapter error parity coverage.

mod common;

use cm_cli::{cli, mcp::tools};
use serde_json::json;

use common::test_store;

#[tokio::test(flavor = "multi_thread")]
async fn cli_and_mcp_share_adapter_error_strings() {
    let (store, _dir) = test_store().await;

    let cli_error = cli::get::run(&store, vec![], false)
        .await
        .unwrap_err()
        .to_string();
    let mcp_error = tools::cx_get(&store, &json!({ "ids": [] }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "get empty ids");

    let cli_error = cli::update::run(
        &store,
        "01950000-0000-7000-8000-000000000000".to_owned(),
        None,
        None,
        None,
        None,
        false,
    )
    .await
    .unwrap_err()
    .to_string();
    let mcp_error = tools::cx_update(
        &store,
        &json!({ "id": "01950000-0000-7000-8000-000000000000" }),
    )
    .await
    .unwrap_err();
    assert_eq!(cli_error, mcp_error, "update requires a field");

    let cli_error = cli::browse::run(
        &store,
        None,
        Some("workspace".to_owned()),
        None,
        None,
        false,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
    )
    .await
    .unwrap_err()
    .to_string();
    let mcp_error = tools::cx_browse(&store, &json!({ "scope_path": "workspace" }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "browse invalid scope path");

    let cli_error = cli::browse::run(
        &store,
        None,
        None,
        None,
        Some(" ".to_owned()),
        false,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
    )
    .await
    .unwrap_err()
    .to_string();
    let mcp_error = tools::cx_browse(&store, &json!({ "cwd": " " }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "browse empty cwd");

    let cli_error = cli::recall::run(
        &store,
        Some("query".to_owned()),
        Some("workspace".to_owned()),
        vec![],
        vec![],
        None,
        None,
        false,
    )
    .await
    .unwrap_err()
    .to_string();
    let mcp_error = tools::cx_recall(&store, &json!({ "query": "query", "scope": "workspace" }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "recall invalid scope path");

    let cli_error = cli::stats::run(&store, Some("recent".to_owned()), false)
        .await
        .unwrap_err()
        .to_string();
    let mcp_error = tools::cx_stats(&store, &json!({ "tag_sort": "recent" }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "stats invalid tag sort");

    let cli_error = cli::deposit::run(
        &store,
        "[]".to_owned(),
        None,
        Some("global".to_owned()),
        None,
        false,
    )
    .await
    .unwrap_err()
    .to_string();
    let mcp_error = tools::cx_deposit(&store, &json!({ "exchanges": [], "scope_path": "global" }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "deposit empty exchanges");

    let cli_error = cli::forget::run(&store, vec![])
        .await
        .unwrap_err()
        .to_string();
    let mcp_error = tools::cx_forget(&store, &json!({ "ids": [] }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "forget empty ids");

    let cli_error = cli::export::run(&store, None, Some("yaml".to_owned()))
        .await
        .unwrap_err()
        .to_string();
    let mcp_error = tools::cx_export(&store, &json!({ "format": "yaml" }))
        .await
        .unwrap_err();
    assert_eq!(cli_error, mcp_error, "export unsupported format");
}
