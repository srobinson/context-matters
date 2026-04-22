//! Integration tests for mutating and end-to-end `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp-file SQLite database, runs migrations,
//! and exercises tool handlers through the public `tools::cx_*` functions.
//! This validates the full stack: JSON params -> tool handler -> ContextStore -> SQLite.

mod common;

use cm_core::{ContextStore, ScopePath};
use serde_json::{Value, json};

use cm_cli::mcp::tools;
use common::{count_row_lines, create_global, extract_stored_id, test_store};

// ── cx_store tests ──────────────────────────────────────────────

async fn store_metadata_error(metadata: Value) -> String {
    let Some(metadata) = metadata.as_object() else {
        panic!("metadata payload must be an object");
    };
    let mut payload = serde_json::Map::from_iter([
        ("title".to_owned(), json!("Bad metadata")),
        ("body".to_owned(), json!("Body.")),
        ("kind".to_owned(), json!("fact")),
    ]);
    payload.extend(metadata.clone());

    let (store, _dir) = test_store().await;
    tools::cx_store(&store, &Value::Object(payload))
        .await
        .unwrap_err()
}

async fn update_metadata_error(metadata: Value) -> String {
    let (store, _dir) = test_store().await;
    tools::cx_update(
        &store,
        &json!({
            "id": "01950000-0000-7000-8000-000000000000",
            "meta": metadata
        }),
    )
    .await
    .unwrap_err()
}

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
async fn store_and_update_share_invalid_metadata_errors() {
    let cases = [
        json!({ "confidence": "maybe" }),
        json!({ "expires_at": "not-a-date" }),
        json!({ "tags": ["valid", 42] }),
    ];

    for metadata in cases {
        let store_error = store_metadata_error(metadata.clone()).await;
        let update_error = update_metadata_error(metadata).await;

        assert_eq!(store_error, update_error);
    }
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
    // by design, scope/kind never change, title lives in the entry body.
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
async fn e2e_recall_id_round_trips_through_get() {
    // ALP-1767 regression: after ripping the short-id prefix input path
    // out of `cx_get`, only full hyphenated UUIDv7s parse. This test
    // proves the id that `cx_recall` surfaces in its structured payload
    // is directly usable against `cx_get` with no trimming, padding, or
    // hex manipulation in between.
    //
    // The YAML text channel does not render the uuid in row lines, so
    // the id must come from the JSON `structuredContent` payload that
    // the dual-channel response carries alongside the text.
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({
            "title": "Round trip probe",
            "body": "Round trip probe body with unique marker widgetflange.",
            "kind": "reference"
        }),
    )
    .await
    .unwrap();

    let recall = tools::cx_recall(&store, &json!({"query": "widgetflange"}))
        .await
        .unwrap();
    let structured = recall
        .structured
        .as_ref()
        .expect("cx_recall must emit a structured payload");
    let entries = structured["entries"]
        .as_array()
        .expect("structured.entries must be an array");
    assert!(!entries.is_empty(), "recall must return at least one row");
    let recall_id = entries[0]["id"]
        .as_str()
        .expect("recall row must carry a string id")
        .to_owned();

    // The id returned by recall must parse as a full UUID and route
    // cleanly through cx_get without any pre-processing by the caller.
    assert!(
        uuid::Uuid::parse_str(&recall_id).is_ok(),
        "recall id `{recall_id}` must be a full hyphenated UUIDv7"
    );

    let get = tools::cx_get(&store, &json!({"ids": [&recall_id]}))
        .await
        .unwrap()
        .text;
    assert!(get.contains("found: 1"), "expected found: 1, got:\n{get}");
    assert!(!get.contains("missing: ["));
    assert!(get.contains("widgetflange"));
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

    // Browse should not include it by default. Rows render the title
    // on the list line (no short-id column after ALP-1767 phase 2),
    // so substring-check the unique title instead.
    let browse = tools::cx_browse(&store, &json!({})).await.unwrap().text;
    assert!(!browse.contains("Will vanish"));

    // Browse with include_superseded should include it.
    let browse2 = tools::cx_browse(&store, &json!({"include_superseded": true}))
        .await
        .unwrap()
        .text;
    assert!(browse2.contains("Will vanish"));
}
