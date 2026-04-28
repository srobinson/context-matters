//! Integration tests for read-oriented `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp-file SQLite database, runs migrations,
//! and exercises tool handlers through the public `tools::cx_*` functions.
//! This validates the full stack: JSON params -> tool handler -> ContextStore -> SQLite.

mod common;

use cm_capabilities::recall::RECALL_SCOPE_DEFAULT_ADVISORY;
use cm_capabilities::validation::{parse_kind, parse_tag_sort};
use cm_cli::mcp::tools;
use common::{
    count_row_lines, create_global, extract_browse_cursor, extract_stored_id, test_store,
};
use serde_json::json;

// -- cx_recall tests ---------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn recall_with_query_searches_fts() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({
            "title": "SQLx migration guide",
            "body": "Run sqlx migrate to apply pending migrations.",
            "kind": "reference"
        }),
    )
    .await
    .unwrap();

    let result = tools::cx_recall(
        &store,
        &json!({
            "query": "sqlx migrate"
        }),
    )
    .await
    .unwrap()
    .text;
    // YAML envelope: routing: search header, at least one row, no full body key.
    assert!(result.contains("routing: search"));
    assert!(result.contains("SQLx migration guide"));
    assert!(count_row_lines(&result) >= 1);
    assert!(!result.contains("\n    body:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_without_scope_surfaces_structured_advisory() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "Global fact", "body": "Fact.", "kind": "fact"}),
    )
    .await
    .unwrap();

    let result = tools::cx_recall(&store, &json!({})).await.unwrap();
    let structured = result
        .structured
        .as_ref()
        .expect("cx_recall emits structured content");

    assert_eq!(
        structured["advisories"],
        json!([RECALL_SCOPE_DEFAULT_ADVISORY])
    );
    assert_eq!(structured["header"]["scope_chain"], json!(["global"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_without_query_uses_scope_resolution() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({
            "title": "Global preference",
            "body": "Always use rfc3339 timestamps.",
            "kind": "preference"
        }),
    )
    .await
    .unwrap();

    tools::cx_store(
        &store,
        &json!({
            "title": "Project fact",
            "body": "Helioy uses monorepo structure.",
            "kind": "fact",
            "scope": "global/project:helioy"
        }),
    )
    .await
    .unwrap();

    let result = tools::cx_recall(
        &store,
        &json!({
            "scope": "global/project:helioy"
        }),
    )
    .await
    .unwrap()
    .text;
    // Scope-resolve routing walks ancestors: chain renders both levels and
    // the body returns both project-level and global entries.
    assert!(result.contains("scope_chain: [global/project:helioy, global]"));
    assert!(count_row_lines(&result) >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_filters_by_kinds() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "A fact", "body": "Fact body.", "kind": "fact"}),
    )
    .await
    .unwrap();
    tools::cx_store(
        &store,
        &json!({"title": "A decision", "body": "Decision body.", "kind": "decision"}),
    )
    .await
    .unwrap();

    let result = tools::cx_recall(
        &store,
        &json!({
            "kinds": ["fact"]
        }),
    )
    .await
    .unwrap()
    .text;
    // Every row should carry `kind: fact` in its trailing comment; no
    // `kind: decision` should appear anywhere in the rendered body.
    assert!(result.contains("A fact"));
    assert!(!result.contains("A decision"));
    assert!(result.contains("kind: fact"));
    assert!(!result.contains("kind: decision"));
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_respects_max_tokens_budget() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..10 {
        tools::cx_store(
            &store,
            &json!({
                "title": format!("Entry {i}"),
                "body": format!("Body content for entry number {i} with some padding text to ensure tokens."),
                "kind": "fact"
            }),
        )
        .await
        .unwrap();
    }

    let result = tools::cx_recall(
        &store,
        &json!({
            "max_tokens": 50
        }),
    )
    .await
    .unwrap()
    .text;
    // With a very small budget, should return fewer than all 10 rows.
    assert!(count_row_lines(&result) < 10);
    // Header surfaces the budget so callers see how the clamp was applied.
    assert!(result.contains("of 50 budget"));
}

// -- cx_get tests ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn get_returns_full_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({
            "title": "Full body test",
            "body": "This is the complete body content that should be returned.",
            "kind": "fact"
        }),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    let result = tools::cx_get(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(result.contains("found: 1"));
    assert!(!result.contains("missing: ["));
    assert!(result.contains("complete body"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_reports_missing_ids() {
    let (store, _dir) = test_store().await;

    let fake_id = "01950000-0000-7000-8000-000000000000";
    let result = tools::cx_get(&store, &json!({"ids": [fake_id]}))
        .await
        .unwrap()
        .text;
    assert!(result.contains("found: 0"));
    assert!(result.contains(&format!("missing: [{fake_id}]")));
    assert!(result.contains("1 missing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_rejects_empty_ids() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_get(&store, &json!({"ids": []})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_rejects_invalid_uuid_input() {
    // `cx_get` only accepts full hyphenated UUIDv7. Non-UUID input
    // surfaces as a crisp validation error instead of silently missing
    // rows or running a prefix scan.
    let (store, _dir) = test_store().await;
    let result = tools::cx_get(&store, &json!({"ids": ["not-a-uuid"]})).await;
    let err = result.expect_err("non-uuid input must be rejected");
    assert!(
        err.contains("invalid UUID"),
        "expected invalid-uuid error, got: {err}"
    );
}

// -- cx_browse tests ---------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn browse_returns_paginated_results() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..5 {
        tools::cx_store(
            &store,
            &json!({
                "title": format!("Browse entry {i}"),
                "body": format!("Body {i}"),
                "kind": "fact"
            }),
        )
        .await
        .unwrap();
    }

    let result = tools::cx_browse(&store, &json!({"limit": 2}))
        .await
        .unwrap()
        .text;
    assert!(result.contains("total: 5"));
    assert!(result.contains("returned: 2"));
    assert_eq!(count_row_lines(&result), 2);

    // Pagination trailer renders as `# N more - cx_browse(cursor="X", limit=L) to page`.
    let cursor = extract_browse_cursor(&result).expect("cursor in pagination trailer");
    let result2 = tools::cx_browse(&store, &json!({"limit": 2, "cursor": cursor}))
        .await
        .unwrap()
        .text;
    assert_eq!(count_row_lines(&result2), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_kind() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "A fact", "body": "Fact.", "kind": "fact"}),
    )
    .await
    .unwrap();
    tools::cx_store(
        &store,
        &json!({"title": "A lesson", "body": "Lesson.", "kind": "lesson"}),
    )
    .await
    .unwrap();

    let result = tools::cx_browse(&store, &json!({"kind": "lesson"}))
        .await
        .unwrap()
        .text;
    // Filter leaves one row: total + returned both equal 1, and the
    // reconstructed filter header echoes the kind back to the caller.
    assert!(result.contains("total: 1"));
    assert!(result.contains("returned: 1"));
    assert!(result.contains("kind=lesson"));
    assert!(result.contains("A lesson"));
    assert!(!result.contains("A fact"));
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_rejects_invalid_kind_with_capability_error() {
    let (store, _dir) = test_store().await;
    let err = tools::cx_browse(&store, &json!({"kind": "memo"}))
        .await
        .unwrap_err();
    assert_eq!(err, parse_kind("memo").unwrap_err());
}

// -- cx_stats tests ----------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn stats_returns_correct_counts() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "Fact 1", "body": "Body 1.", "kind": "fact"}),
    )
    .await
    .unwrap();
    tools::cx_store(
        &store,
        &json!({"title": "Fact 2", "body": "Body 2.", "kind": "fact"}),
    )
    .await
    .unwrap();
    tools::cx_store(
        &store,
        &json!({"title": "Decision 1", "body": "Body 3.", "kind": "decision"}),
    )
    .await
    .unwrap();

    let result = tools::cx_stats(&store, &json!({})).await.unwrap().text;
    // Counters: 3 active entries. Kinds block carries a `fact  2` row
    // and a `decision  1` row (column-aligned by max-kind-width).
    assert!(result.contains("active: 3"));
    assert!(result.contains("fact"));
    assert!(result.contains("decision"));
    // Scope tree section exists and has at least the global root row
    // (rendered by the test fixture label "Global").
    assert!(result.contains("scope_tree:"));
    assert!(result.contains("Global"));
}

#[tokio::test(flavor = "multi_thread")]
async fn stats_rejects_invalid_tag_sort_with_capability_error() {
    let (store, _dir) = test_store().await;
    let err = tools::cx_stats(&store, &json!({"tag_sort": "recent"}))
        .await
        .unwrap_err();
    assert_eq!(err, parse_tag_sort("recent").unwrap_err());
}
