//! MCP adapter tests for smart browse scope inputs.
//!
//! Capability-level resolver behavior is covered in cm-capabilities.
//! These tests focus on the cx_browse tool boundary: parameter mapping,
//! default MCP inferred scope, removed input errors, text projection, and
//! structuredContent resolution metadata.

mod common;

use cm_cli::mcp::tools;
use serde_json::json;

use common::{create_global, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn browse_defaults_to_cwd_inferred_scope_for_mcp() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "Global fact", "body": "Fact.", "kind": "fact"}),
    )
    .await
    .unwrap();

    let result = tools::cx_browse(&store, &json!({})).await.unwrap();

    assert!(
        result.text.contains("query: scope=cwd_inferred"),
        "default MCP browse should disclose cwd_inferred scope:\n{}",
        result.text,
    );
    assert!(
        result.text.contains("resolution:"),
        "cwd_inferred browse should render compact resolution metadata:\n{}",
        result.text,
    );
    let structured = result
        .structured
        .as_ref()
        .expect("cx_browse emits structured content");
    assert_eq!(structured["resolution"]["requested_scope"], "cwd_inferred");
    assert_eq!(structured["resolution"]["resolved_scope"], "global");
    assert_eq!(structured["resolution"]["confidence"], "very_low");
    assert_eq!(
        structured["advisory"],
        "no scope specified, using scope='cwd_inferred' to infer the local scope. run `cm stats` to list all scopes."
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_cwd_inferred_scope_resolves_repo_and_returns_resolution() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let repo_scope = "global/project:helioy/repo:context-matters";

    tools::cx_store(
        &store,
        &json!({"title": "Global fact", "body": "Fact.", "kind": "fact"}),
    )
    .await
    .unwrap();
    tools::cx_store(
        &store,
        &json!({
            "title": "Repo fact",
            "body": "Repo-local fact.",
            "kind": "fact",
            "scope": repo_scope
        }),
    )
    .await
    .unwrap();

    let result = tools::cx_browse(
        &store,
        &json!({
            "scope": "cwd_inferred",
            "cwd": "/tmp/helioy/context-matters",
            "limit": 20
        }),
    )
    .await
    .unwrap();

    assert!(
        result.text.contains("query: scope=cwd_inferred"),
        "\n{}",
        result.text
    );
    assert!(
        result
            .text
            .contains(&format!("resolved_scope: {repo_scope}")),
        "\n{}",
        result.text,
    );
    assert!(
        result.text.contains("confidence: high"),
        "\n{}",
        result.text
    );
    assert!(result.text.contains("Repo fact"), "\n{}", result.text);
    assert!(
        !result.text.contains("Global fact"),
        "cwd_inferred browse must use the resolved exact repo scope:\n{}",
        result.text,
    );
    assert!(
        result.text.len() < 1200,
        "cwd_inferred browse text should stay compact:\n{}",
        result.text,
    );

    let structured = result
        .structured
        .as_ref()
        .expect("cx_browse emits structured content");
    assert_eq!(structured["resolution"]["requested_scope"], "cwd_inferred");
    assert_eq!(structured["resolution"]["resolved_scope"], repo_scope);
    assert_eq!(structured["resolution"]["scope_mode"], "resolved");
    assert_eq!(structured["resolution"]["confidence"], "high");
    assert_eq!(structured["header"]["scope"], repo_scope);
    assert_eq!(structured["entries"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_stays_exact_without_resolution() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "Global fact", "body": "Fact.", "kind": "fact"}),
    )
    .await
    .unwrap();
    tools::cx_store(
        &store,
        &json!({
            "title": "Project fact",
            "body": "Project-local fact.",
            "kind": "fact",
            "scope": "global/project:helioy"
        }),
    )
    .await
    .unwrap();

    let result = tools::cx_browse(
        &store,
        &json!({"scope": "global/project:helioy", "limit": 20}),
    )
    .await
    .unwrap();

    assert!(
        result.text.contains("query: scope=global/project:helioy"),
        "\n{}",
        result.text,
    );
    assert!(result.text.contains("Project fact"), "\n{}", result.text);
    assert!(!result.text.contains("Global fact"), "\n{}", result.text);
    assert!(
        !result.text.contains("resolution:"),
        "explicit scope should not infer scope:\n{}",
        result.text,
    );
    let structured = result
        .structured
        .as_ref()
        .expect("cx_browse emits structured content");
    assert!(structured.get("resolution").is_none());
    assert_eq!(structured["header"]["scope"], "global/project:helioy");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_rejects_removed_scope_path() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let err = tools::cx_browse(&store, &json!({"scope_path": "global"}))
        .await
        .unwrap_err();

    assert!(
        err.contains("scope_path has been removed"),
        "unexpected error: {err}",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_rejects_removed_auto_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let err = tools::cx_browse(&store, &json!({"scope": "auto"}))
        .await
        .unwrap_err();

    assert!(
        err.contains("scope='auto' has been removed"),
        "unexpected error: {err}",
    );
}
