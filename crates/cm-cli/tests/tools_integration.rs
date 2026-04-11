//! Integration tests for the 9 `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp-file SQLite database, runs migrations,
//! and exercises tool handlers through the public `tools::cx_*` functions.
//! This validates the full stack: JSON params -> tool handler -> ContextStore -> SQLite.

mod common;

use cm_core::{ContextStore, ScopePath};
use serde_json::{Value, json};

use cm_cli::mcp::tools;
use common::{
    count_row_lines, create_global, extract_browse_cursor, extract_stored_id, test_store,
};

// ── cx_store tests ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn store_creates_entry_at_global_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Test fact",
            "body": "This is a test fact body.",
            "kind": "fact"
        }),
    )
    .await;

    let text = result.unwrap().text;
    assert!(text.contains("scope: global"));
    assert!(text.contains("kind: fact"));
    // The YAML envelope carries the full uuid on its `stored:` line; the
    // helper both asserts the line exists and returns the id for reuse.
    assert!(extract_stored_id(&text).len() > 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_auto_creates_scope_chain() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Repo-level decision",
            "body": "Use sqlx for database access.",
            "kind": "decision",
            "scope_path": "global/project:helioy/repo:nancyr"
        }),
    )
    .await;

    let text = result.unwrap().text;
    assert!(text.contains("scope: global/project:helioy/repo:nancyr"));

    // Verify ancestor scopes were created
    let project_scope = store
        .get_scope(&ScopePath::parse("global/project:helioy").unwrap())
        .await
        .unwrap();
    assert_eq!(project_scope.label, "helioy");
}

#[tokio::test(flavor = "multi_thread")]
async fn store_with_supersedes() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r1 = tools::cx_store(
        &store,
        &json!({
            "title": "Original decision",
            "body": "Use diesel for ORM.",
            "kind": "decision"
        }),
    )
    .await
    .unwrap()
    .text;
    let old_id = extract_stored_id(&r1);

    let r2 = tools::cx_store(
        &store,
        &json!({
            "title": "Updated decision",
            "body": "Use sqlx instead of diesel.",
            "kind": "decision",
            "supersedes": &old_id
        }),
    )
    .await
    .unwrap()
    .text;
    // The ack carries `superseded: <old_id>` right after the new `stored:`
    // line when `supersedes` was passed on the request.
    assert!(r2.contains(&format!("superseded: {old_id}")));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_empty_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "",
            "body": "Some body",
            "kind": "fact"
        }),
    )
    .await;
    assert!(result.is_err() || result.unwrap().text.contains("empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_invalid_kind() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Test",
            "body": "Test body",
            "kind": "bogus"
        }),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid kind"));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_invalid_scope_path() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_store(
        &store,
        &json!({
            "title": "Test",
            "body": "Test body",
            "kind": "fact",
            "scope_path": "not/valid"
        }),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid scope_path"));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_detects_duplicate_content() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let args = json!({
        "title": "Dup test",
        "body": "Identical body content.",
        "kind": "fact"
    });

    tools::cx_store(&store, &args).await.unwrap();
    let result = tools::cx_store(&store, &args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Duplicate content"));
}

// ── cx_recall tests ─────────────────────────────────────────────

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
            "scope_path": "global/project:helioy"
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

    // Create several entries
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

// ── cx_get tests ────────────────────────────────────────────────

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

// ── cx_browse tests ─────────────────────────────────────────────

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

// ── cx_update tests ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn update_changes_title_and_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Original", "body": "Original body.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    let result = tools::cx_update(
        &store,
        &json!({
            "id": &id,
            "title": "Updated title",
            "body": "Updated body content."
        }),
    )
    .await
    .unwrap()
    .text;
    // `format_update_ack` emits just `updated: <id>` + `content_hash: <prefix>`
    // by design — scope/kind never change, title lives in the entry body.
    // The body/title round-trip is covered by `e2e_store_update_verify`.
    assert!(result.contains(&format!("updated: {id}")));
    assert!(result.contains("content_hash: "));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_rejects_no_fields() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_update(
        &store,
        &json!({"id": "01950000-0000-7000-8000-000000000000"}),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least one field"));
}

// ── cx_forget tests ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn forget_soft_deletes_entry() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "To forget", "body": "Will be forgotten.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    let result = tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    // `format_forget_ack` renders the three disposition counters
    // unconditionally, each on its own line.
    assert!(result.contains("forgotten: 1"));
    assert!(result.contains("already_inactive: 0"));

    // Recall searches by short-id prefix against the rendered row list.
    let recall = tools::cx_recall(&store, &json!({})).await.unwrap().text;
    let sid_prefix = &id[..8];
    assert!(!recall.contains(sid_prefix));
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_reports_already_inactive() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Double forget", "body": "Body.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap();
    let result = tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(result.contains("forgotten: 0"));
    assert!(result.contains("already_inactive: 1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_reports_not_found() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_forget(
        &store,
        &json!({"ids": ["01950000-0000-7000-8000-000000000000"]}),
    )
    .await
    .unwrap()
    .text;
    assert!(result.contains("not_found: 1"));
}

// ── cx_deposit tests ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn deposit_creates_exchange_entries() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_deposit(
        &store,
        &json!({
            "exchanges": [
                {"user": "How do I parse JSON?", "assistant": "Use serde_json::from_str."},
                {"user": "What about errors?", "assistant": "Use the ? operator with Result."}
            ]
        }),
    )
    .await
    .unwrap()
    .text;
    // `format_deposit_ack` pluralises `exchange` and, without a summary,
    // renders an inline `entry_ids: [id1, id2]` list of 8-char shorts.
    assert!(result.contains("deposited: 2 exchanges"));
    assert!(result.contains("entry_ids: ["));
    // Two ids in the list means one comma separator; zero summary means
    // no `summary:` line at all.
    let id_line = result
        .lines()
        .find(|l| l.starts_with("entry_ids: ["))
        .expect("entry_ids line present");
    assert_eq!(id_line.matches(',').count(), 1);
    assert!(!result.contains("summary:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_with_summary_creates_relations() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_deposit(
        &store,
        &json!({
            "exchanges": [
                {"user": "What is Rust?", "assistant": "A systems programming language."}
            ],
            "summary": "Discussed Rust programming language basics."
        }),
    )
    .await
    .unwrap()
    .text;
    // Single exchange renders singular `exchange` (no `s`). With a summary
    // present, `format_deposit_ack` suppresses the per-entry `entry_ids`
    // list and surfaces the summary's full uuid instead.
    assert!(result.contains("deposited: 1 exchange\n"));
    assert!(result.contains("summary: "));
    assert!(!result.contains("entry_ids: ["));
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_rejects_empty_exchanges() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_deposit(&store, &json!({"exchanges": []})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

// ── cx_stats tests ──────────────────────────────────────────────

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

// ── cx_export tests ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn export_returns_all_entries() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({"title": "Export test", "body": "Body.", "kind": "fact"}),
    )
    .await
    .unwrap();

    let result = tools::cx_export(&store, &json!({})).await.unwrap();
    // cx_export emits structured-only: text channel is empty, the JSON
    // payload lives in the structured channel for backup/restore fidelity.
    let resp: Value = result
        .structured
        .expect("cx_export must emit a structured payload");
    assert_eq!(resp["count"], 1);
    assert!(resp["exported_at"].as_str().is_some());
    assert!(!resp["scopes"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn export_rejects_unsupported_format() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_export(&store, &json!({"format": "csv"})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unsupported export format"));
}

// ── End-to-end flow tests ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn e2e_store_recall_get_flow() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Store
    let r = tools::cx_store(
        &store,
        &json!({
            "title": "E2E test entry",
            "body": "End-to-end test content for verification.",
            "kind": "lesson",
            "tags": ["testing", "e2e"]
        }),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    // Recall by query (use a term without hyphens to avoid FTS5 parsing issues)
    let recall = tools::cx_recall(&store, &json!({"query": "verification"}))
        .await
        .unwrap()
        .text;
    assert!(recall.contains("routing: search"));
    assert!(count_row_lines(&recall) >= 1);

    // Get full content
    let get = tools::cx_get(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(get.contains("found: 1"));
    assert!(get.contains("End-to-end"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_store_update_verify() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Before update", "body": "Original.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    tools::cx_update(
        &store,
        &json!({"id": &id, "title": "After update", "body": "Modified."}),
    )
    .await
    .unwrap();

    let get = tools::cx_get(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(get.contains("title: After update"));
    assert!(get.contains("Modified."));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_store_forget_exclusion() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Will vanish", "body": "Gone soon.", "kind": "observation"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap();

    // Browse should not include it by default; rows render only the
    // short-id prefix, so substring-check the first 8 bytes of the uuid.
    let sid_prefix = &id[..8];
    let browse = tools::cx_browse(&store, &json!({})).await.unwrap().text;
    assert!(!browse.contains(sid_prefix));

    // Browse with include_superseded should include it.
    let browse2 = tools::cx_browse(&store, &json!({"include_superseded": true}))
        .await
        .unwrap()
        .text;
    assert!(browse2.contains(sid_prefix));
}
