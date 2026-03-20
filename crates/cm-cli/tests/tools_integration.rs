//! Integration tests for the 9 `cx_*` MCP tool handlers.
//!
//! Each test creates an isolated temp-file SQLite database, runs migrations,
//! and exercises tool handlers through the public `tools::cx_*` functions.
//! This validates the full stack: JSON params -> tool handler -> ContextStore -> SQLite.

use cm_core::{ContextStore, MutationSource, NewScope, ScopePath, WriteContext};
use cm_store::{CmStore, schema};
use serde_json::{Value, json};

use cm_cli::mcp::tools;

/// Create an isolated store backed by a temp-file SQLite database.
async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();

    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

/// Create the global scope in the store.
async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Global".to_owned(),
                meta: None,
            },
            &WriteContext::new(MutationSource::Mcp),
        )
        .await
        .unwrap();
}

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

    let text = result.unwrap();
    let resp: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(resp["scope_path"], "global");
    assert_eq!(resp["kind"], "fact");
    assert_eq!(resp["message"], "Entry stored.");
    assert!(resp["id"].as_str().unwrap().len() > 10);
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

    let text = result.unwrap();
    let resp: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(resp["scope_path"], "global/project:helioy/repo:nancyr");

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
    .unwrap();
    let resp1: Value = serde_json::from_str(&r1).unwrap();
    let old_id = resp1["id"].as_str().unwrap();

    let r2 = tools::cx_store(
        &store,
        &json!({
            "title": "Updated decision",
            "body": "Use sqlx instead of diesel.",
            "kind": "decision",
            "supersedes": old_id
        }),
    )
    .await
    .unwrap();
    let resp2: Value = serde_json::from_str(&r2).unwrap();
    assert_eq!(resp2["superseded"], old_id);
    assert!(resp2["message"].as_str().unwrap().contains("Superseded"));
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
    assert!(result.is_err() || result.unwrap().contains("empty"));
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
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert!(resp["returned"].as_u64().unwrap() >= 1);
    // Should have snippet, not full body
    let first = &resp["results"][0];
    assert!(first.get("snippet").is_some());
    assert!(first.get("body").is_none());
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
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    // Should return both project-level and global entries
    assert!(resp["returned"].as_u64().unwrap() >= 2);
    assert_eq!(resp["scope_chain"][0], "global/project:helioy");
    assert_eq!(resp["scope_chain"][1], "global");
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
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    for entry in resp["results"].as_array().unwrap() {
        assert_eq!(entry["kind"], "fact");
    }
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
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    // With a very small budget, should return fewer than all 10
    assert!(resp["returned"].as_u64().unwrap() < 10);
    // Per-entry token estimates should be present
    let first = &resp["results"][0];
    assert!(first["token_estimate"].as_u64().unwrap() > 0);
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    let result = tools::cx_get(&store, &json!({"ids": [id]})).await.unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["found"], 1);
    assert_eq!(resp["missing"], 0);
    assert!(
        resp["entries"][0]["body"]
            .as_str()
            .unwrap()
            .contains("complete body")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_reports_missing_ids() {
    let (store, _dir) = test_store().await;

    let fake_id = "01950000-0000-7000-8000-000000000000";
    let result = tools::cx_get(&store, &json!({"ids": [fake_id]}))
        .await
        .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["found"], 0);
    assert_eq!(resp["missing"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_rejects_empty_ids() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_get(&store, &json!({"ids": []})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_rejects_invalid_uuid() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_get(&store, &json!({"ids": ["not-a-uuid"]})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid UUID"));
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
        .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["entries"].as_array().unwrap().len(), 2);
    assert_eq!(resp["total"], 5);
    assert_eq!(resp["has_more"], true);
    assert!(resp["next_cursor"].as_str().is_some());

    // Fetch next page
    let cursor = resp["next_cursor"].as_str().unwrap();
    let result2 = tools::cx_browse(&store, &json!({"limit": 2, "cursor": cursor}))
        .await
        .unwrap();
    let resp2: Value = serde_json::from_str(&result2).unwrap();
    assert_eq!(resp2["entries"].as_array().unwrap().len(), 2);
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
        .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["total"], 1);
    assert_eq!(resp["entries"][0]["kind"], "lesson");
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    let result = tools::cx_update(
        &store,
        &json!({
            "id": id,
            "title": "Updated title",
            "body": "Updated body content."
        }),
    )
    .await
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["entry"]["title"], "Updated title");
    assert_eq!(resp["message"], "Entry updated.");
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    let result = tools::cx_forget(&store, &json!({"ids": [id]}))
        .await
        .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["forgotten"], 1);
    assert_eq!(resp["already_inactive"], 0);

    // Verify excluded from recall
    let recall = tools::cx_recall(&store, &json!({})).await.unwrap();
    let recall_resp: Value = serde_json::from_str(&recall).unwrap();
    let ids: Vec<&str> = recall_resp["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(!ids.contains(&id));
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    tools::cx_forget(&store, &json!({"ids": [id]}))
        .await
        .unwrap();
    let result = tools::cx_forget(&store, &json!({"ids": [id]}))
        .await
        .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["forgotten"], 0);
    assert_eq!(resp["already_inactive"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn forget_reports_not_found() {
    let (store, _dir) = test_store().await;

    let result = tools::cx_forget(
        &store,
        &json!({"ids": ["01950000-0000-7000-8000-000000000000"]}),
    )
    .await
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["not_found"], 1);
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
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["deposited"], 2);
    assert_eq!(resp["entry_ids"].as_array().unwrap().len(), 2);
    assert!(resp["summary_id"].is_null());
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
    .unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["deposited"], 1);
    assert!(resp["summary_id"].as_str().is_some());
    assert!(resp["message"].as_str().unwrap().contains("summary"));
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

    let result = tools::cx_stats(&store, &json!({})).await.unwrap();
    let resp: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(resp["active_entries"], 3);
    assert_eq!(resp["entries_by_kind"]["fact"], 2);
    assert_eq!(resp["entries_by_kind"]["decision"], 1);
    assert!(!resp["scope_tree"].as_array().unwrap().is_empty());
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
    let resp: Value = serde_json::from_str(&result).unwrap();
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    // Recall by query (use a term without hyphens to avoid FTS5 parsing issues)
    let recall = tools::cx_recall(&store, &json!({"query": "verification"}))
        .await
        .unwrap();
    let recall_resp: Value = serde_json::from_str(&recall).unwrap();
    assert!(recall_resp["returned"].as_u64().unwrap() >= 1);

    // Get full content
    let get = tools::cx_get(&store, &json!({"ids": [id]})).await.unwrap();
    let get_resp: Value = serde_json::from_str(&get).unwrap();
    assert_eq!(get_resp["found"], 1);
    assert!(
        get_resp["entries"][0]["body"]
            .as_str()
            .unwrap()
            .contains("End-to-end")
    );
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    tools::cx_update(
        &store,
        &json!({"id": id, "title": "After update", "body": "Modified."}),
    )
    .await
    .unwrap();

    let get = tools::cx_get(&store, &json!({"ids": [id]})).await.unwrap();
    let resp: Value = serde_json::from_str(&get).unwrap();
    assert_eq!(resp["entries"][0]["title"], "After update");
    assert_eq!(resp["entries"][0]["body"], "Modified.");
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
    .unwrap();
    let stored: Value = serde_json::from_str(&r).unwrap();
    let id = stored["id"].as_str().unwrap();

    tools::cx_forget(&store, &json!({"ids": [id]}))
        .await
        .unwrap();

    // Browse should not include it by default
    let browse = tools::cx_browse(&store, &json!({})).await.unwrap();
    let resp: Value = serde_json::from_str(&browse).unwrap();
    let ids: Vec<&str> = resp["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(!ids.contains(&id));

    // Browse with include_superseded should include it
    let browse2 = tools::cx_browse(&store, &json!({"include_superseded": true}))
        .await
        .unwrap();
    let resp2: Value = serde_json::from_str(&browse2).unwrap();
    let ids2: Vec<&str> = resp2["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(ids2.contains(&id));
}
