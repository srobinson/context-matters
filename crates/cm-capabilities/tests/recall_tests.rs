//! Capability-level tests for recall routing, filtering, and token budget.
//!
//! Tests exercise `cm_capabilities::recall::recall()` directly against a real
//! SQLite store, covering each routing branch, post-filtering, and budget logic.

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_core::{
    ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};

// ── Helpers ──────────────────────────────────────────────────────

async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Global".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn ensure_scope(store: &CmStore, path: &str) {
    let sp = ScopePath::parse(path).unwrap();
    // Collect ancestors and reverse so we create parent scopes before children
    let ancestors: Vec<&str> = sp.ancestors().collect();
    for ancestor in ancestors.into_iter().rev() {
        let ancestor_path = ScopePath::parse(ancestor).unwrap();
        if store.get_scope(&ancestor_path).await.is_err() {
            let label = ancestor.rsplit('/').next().unwrap_or(ancestor);
            store
                .create_scope(
                    NewScope {
                        path: ancestor_path,
                        label: label.to_owned(),
                        meta: None,
                    },
                    &wctx(),
                )
                .await
                .unwrap();
        }
    }
}

async fn seed_entry(store: &CmStore, title: &str, body: &str, kind: EntryKind) {
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind,
                title: title.to_owned(),
                body: body.to_owned(),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_entry_with_scope(
    store: &CmStore,
    title: &str,
    body: &str,
    kind: EntryKind,
    scope: &str,
) {
    ensure_scope(store, scope).await;
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::parse(scope).unwrap(),
                kind,
                title: title.to_owned(),
                body: body.to_owned(),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_entry_with_tags(
    store: &CmStore,
    title: &str,
    body: &str,
    kind: EntryKind,
    tags: Vec<String>,
) {
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind,
                title: title.to_owned(),
                body: body.to_owned(),
                created_by: "test:seed".to_owned(),
                meta: Some(EntryMeta {
                    tags,
                    ..Default::default()
                }),
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_scoped_tagged_entry(
    store: &CmStore,
    title: &str,
    body: &str,
    kind: EntryKind,
    scope: &str,
    tags: Vec<String>,
) {
    ensure_scope(store, scope).await;
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::parse(scope).unwrap(),
                kind,
                title: title.to_owned(),
                body: body.to_owned(),
                created_by: "test:seed".to_owned(),
                meta: Some(EntryMeta {
                    tags,
                    ..Default::default()
                }),
            },
            &wctx(),
        )
        .await
        .unwrap();
}

// ── Routing: Search ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn routing_search_when_query_provided() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "SQLx migration guide",
        "Run sqlx migrate to apply.",
        EntryKind::Reference,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx migrate".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert!(!result.entries.is_empty());
    assert_eq!(result.entries[0].title, "SQLx migration guide");
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_search_with_scope_filters_to_ancestors() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Global sqlx note",
        "Use sqlx for DB.",
        EntryKind::Fact,
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Project sqlx note",
        "Use sqlx migrations in helioy.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            scope: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert!(result.entries.len() >= 2);
}

// ── Routing: TagScopeWalk ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn routing_tag_scope_walk_when_tags_no_query() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Tagged fact",
        "Body with session tag.",
        EntryKind::Fact,
        vec!["session-log".to_owned()],
    )
    .await;
    seed_entry(&store, "Untagged fact", "No tags here.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            tags: vec!["session-log".to_owned()],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::TagScopeWalk);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Tagged fact");
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_tag_scope_walk_with_scope_walks_ancestors() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Global tagged",
        "At global scope.",
        EntryKind::Fact,
        vec!["infra".to_owned()],
    )
    .await;
    seed_scoped_tagged_entry(
        &store,
        "Project tagged",
        "At project scope.",
        EntryKind::Fact,
        "global/project:helioy",
        vec!["infra".to_owned()],
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            tags: vec!["infra".to_owned()],
            scope: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::TagScopeWalk);
    assert_eq!(result.entries.len(), 2);
}

// ── Routing: ScopeResolve ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn routing_scope_resolve_when_scope_no_query_no_tags() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Global preference",
        "Use rfc3339.",
        EntryKind::Preference,
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Project fact",
        "Helioy uses monorepo.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            scope: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert!(result.entries.len() >= 2);
}

// ── Routing: BrowseFallback ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn routing_browse_fallback_when_no_query_no_scope_no_tags() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact one", "Body one.", EntryKind::Fact).await;
    seed_entry(&store, "Fact two", "Body two.", EntryKind::Decision).await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::BrowseFallback);
    assert_eq!(result.entries.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn routing_browse_fallback_passes_single_kind_to_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Fact body.", EntryKind::Fact).await;
    seed_entry(&store, "A decision", "Decision body.", EntryKind::Decision).await;

    let result = recall(
        &store,
        RecallRequest {
            kinds: vec![EntryKind::Fact],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::BrowseFallback);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].kind, EntryKind::Fact);
}

// ── Kind post-filtering ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn search_post_filters_by_kinds() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Rust fact about sqlx",
        "Use sqlx for queries.",
        EntryKind::Fact,
    )
    .await;
    seed_entry(
        &store,
        "Rust decision about sqlx",
        "We decided to use sqlx.",
        EntryKind::Decision,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            kinds: vec![EntryKind::Fact],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].kind, EntryKind::Fact);
    // candidates_before_filter should reflect both entries matched the search
    assert!(result.candidates_before_filter >= 2);
}

// ── Tag post-filtering ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn search_post_filters_by_tags() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry_with_tags(
        &store,
        "Tagged sqlx note",
        "Use sqlx for database queries.",
        EntryKind::Fact,
        vec!["database".to_owned()],
    )
    .await;
    seed_entry(
        &store,
        "Untagged sqlx note",
        "Also about sqlx usage.",
        EntryKind::Fact,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("sqlx".to_owned()),
            tags: vec!["database".to_owned()],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Tagged sqlx note");
}

// ── Fetch-size compensation ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn fetch_limit_compensates_when_post_filters_active() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Create entries where only some match the kind filter
    for i in 0..6 {
        seed_entry(
            &store,
            &format!("Fact {i}"),
            &format!("Searchable content about topic alpha {i}."),
            EntryKind::Fact,
        )
        .await;
    }
    for i in 0..4 {
        seed_entry(
            &store,
            &format!("Decision {i}"),
            &format!("Searchable content about topic alpha {i}."),
            EntryKind::Decision,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            query: Some("alpha".to_owned()),
            kinds: vec![EntryKind::Fact],
            limit: 5,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // fetch_limit_used should be limit * 3 = 15 (capped at MAX_LIMIT)
    assert_eq!(result.fetch_limit_used, 15);
    // Should still return only facts despite higher fetch limit
    for entry in &result.entries {
        assert_eq!(entry.kind, EntryKind::Fact);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_limit_unchanged_without_post_filters() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Simple fact", "Basic content.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.fetch_limit_used, 10);
}

// ── Token budget ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn token_budget_truncates_results() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..10 {
        seed_entry(
            &store,
            &format!("Entry {i}"),
            &format!("Body content for entry number {i} with padding text to consume tokens."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            max_tokens: Some(50),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // With a tight budget, should return fewer than all 10
    assert!(result.entries.len() < 10);
    assert!(result.token_estimate > 0);
    assert!(result.token_estimate <= 50 + 100); // some slack since first entry always included
}

#[tokio::test(flavor = "multi_thread")]
async fn token_budget_always_includes_first_entry() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Create an entry whose token estimate exceeds the budget
    let long_body = "x".repeat(1000);
    seed_entry(&store, "Large entry", &long_body, EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            max_tokens: Some(1), // impossibly small budget
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Must still include the first entry even if it exceeds budget
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Large entry");
    assert!(result.token_estimate > 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn token_budget_none_returns_all() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..5 {
        seed_entry(
            &store,
            &format!("Entry {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            max_tokens: None,
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 5);
}

// ── Scope chain extraction ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn scope_chain_extracted_from_scope_path() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact", "Body.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            scope: Some(ScopePath::parse("global/project:helioy/repo:cm").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        result.scope_chain,
        vec![
            "global/project:helioy/repo:cm",
            "global/project:helioy",
            "global"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn scope_chain_empty_when_no_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Fact", "Body.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(result.scope_chain.is_empty());
}

// ── Limit enforcement ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn limit_caps_results_after_post_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..10 {
        seed_entry(
            &store,
            &format!("Fact {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = recall(
        &store,
        RecallRequest {
            limit: 3,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 3);
}

// ── Trace metadata ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn result_includes_trace_metadata() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "A fact", "Body.", EntryKind::Fact).await;
    seed_entry(&store, "A decision", "Body.", EntryKind::Decision).await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("body".to_owned()),
            kinds: vec![EntryKind::Fact],
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // candidates_before_filter counts entries before kind filtering
    assert!(result.candidates_before_filter >= 2);
    // fetch_limit_used reflects compensation (limit * 3 because kinds filter is active)
    assert_eq!(result.fetch_limit_used, 60);
    assert_eq!(result.routing, RecallRouting::Search);
}
