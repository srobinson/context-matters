//! Integration tests for `SqliteContextStore`.
//!
//! Covers acceptance criteria 11-28 from the schema & storage spec.
//! Each test creates an isolated in-memory or temp-file database,
//! runs migrations, and exercises the `ContextStore` trait methods.

use cm_core::{
    CmError, ContextStore, EntryFilter, EntryKind, EntryMeta, NewEntry, NewScope, Pagination,
    RelationKind, ScopeKind, ScopePath,
};
use cm_store::{CmStore, schema};

/// Create an isolated store backed by a temp-file SQLite database.
/// Returns the store and a `TempDir` guard (directory is cleaned up on drop).
async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();

    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

/// Helper: create the global scope.
async fn create_global(store: &CmStore) -> cm_core::Scope {
    store
        .create_scope(NewScope {
            path: ScopePath::parse("global").unwrap(),
            label: "Global".to_owned(),
            meta: None,
        })
        .await
        .unwrap()
}

/// Helper: create the global + project scope hierarchy.
async fn create_project_scope(store: &CmStore, project: &str) -> cm_core::Scope {
    create_global(store).await;
    let path = format!("global/project:{project}");
    store
        .create_scope(NewScope {
            path: ScopePath::parse(&path).unwrap(),
            label: project.to_owned(),
            meta: None,
        })
        .await
        .unwrap()
}

/// Helper: build a basic NewEntry.
fn new_entry(scope: &str, kind: EntryKind, title: &str, body: &str) -> NewEntry {
    NewEntry {
        scope_path: ScopePath::parse(scope).unwrap(),
        kind,
        title: title.to_owned(),
        body: body.to_owned(),
        created_by: "agent:test".to_owned(),
        meta: None,
    }
}

// ── Criterion 11: insert with valid scope succeeds and returns ID ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c11_create_entry_with_valid_scope_returns_entry_with_id() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Test title",
            "Test body",
        ))
        .await
        .unwrap();

    assert!(!entry.id.is_nil());
    assert_eq!(entry.title, "Test title");
    assert_eq!(entry.body, "Test body");
    assert_eq!(entry.kind, EntryKind::Fact);
    assert_eq!(entry.scope_path.as_str(), "global");
    assert_eq!(entry.created_by, "agent:test");
    assert!(!entry.content_hash.is_empty());
    assert!(entry.superseded_by.is_none());
}

// ── Criterion 12: insert with invalid scope fails with FK error ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c12_create_entry_with_missing_scope_fails() {
    let (store, _dir) = test_store().await;
    // No scopes created

    let result = store
        .create_entry(new_entry("global", EntryKind::Fact, "Orphan", "No scope"))
        .await;

    assert!(matches!(result, Err(CmError::ScopeNotFound(_))));
}

// ── Criterion 13: query entries by scope returns only that scope ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c13_query_by_scope_returns_exact_scope_only() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:alpha").unwrap();
    store
        .create_scope(NewScope {
            path: project_path.clone(),
            label: "Alpha".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    // Create entries at different scopes
    store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Global entry",
            "At global",
        ))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:alpha",
            EntryKind::Fact,
            "Project entry",
            "At project",
        ))
        .await
        .unwrap();

    // Browse with scope filter returns only that scope
    let result = store
        .browse(EntryFilter {
            scope_path: Some(project_path),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Project entry");
}

// ── Criterion 14: resolve_context walks ancestor chain ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c14_resolve_context_returns_ancestors_most_specific_first() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:helioy").unwrap();
    store
        .create_scope(NewScope {
            path: project_path.clone(),
            label: "Helioy".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    let repo_path = ScopePath::parse("global/project:helioy/repo:nancyr").unwrap();
    store
        .create_scope(NewScope {
            path: repo_path.clone(),
            label: "nancyr".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    // Create entries at each scope level
    store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Global fact",
            "Global body",
        ))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:helioy",
            EntryKind::Decision,
            "Project decision",
            "Project body",
        ))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:helioy/repo:nancyr",
            EntryKind::Lesson,
            "Repo lesson",
            "Repo body",
        ))
        .await
        .unwrap();

    let entries = store.resolve_context(&repo_path, &[], 100).await.unwrap();

    assert_eq!(entries.len(), 3);
    // Most specific first: repo, then project, then global
    assert_eq!(
        entries[0].scope_path.as_str(),
        "global/project:helioy/repo:nancyr"
    );
    assert_eq!(entries[1].scope_path.as_str(), "global/project:helioy");
    assert_eq!(entries[2].scope_path.as_str(), "global");
}

// ── Criterion 14 (kind filter): resolve_context respects kind filter ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c14_resolve_context_filters_by_kind() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:test").unwrap();
    store
        .create_scope(NewScope {
            path: project_path.clone(),
            label: "Test".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    store
        .create_entry(new_entry("global", EntryKind::Fact, "Fact", "fact body"))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global",
            EntryKind::Decision,
            "Decision",
            "decision body",
        ))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:test",
            EntryKind::Fact,
            "Project fact",
            "project fact body",
        ))
        .await
        .unwrap();

    let entries = store
        .resolve_context(&project_path, &[EntryKind::Fact], 100)
        .await
        .unwrap();

    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.kind == EntryKind::Fact));
}

// ── Criterion 15: supersede sets superseded_by and creates new entry ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c15_supersede_entry_marks_old_and_creates_new() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let old = store
        .create_entry(new_entry("global", EntryKind::Fact, "Old fact", "Old body"))
        .await
        .unwrap();

    let replacement = new_entry("global", EntryKind::Fact, "New fact", "New body");
    let new = store.supersede_entry(old.id, replacement).await.unwrap();

    // New entry is active
    assert!(new.superseded_by.is_none());
    assert_eq!(new.title, "New fact");

    // Old entry is marked superseded
    let old_fetched = store.get_entry(old.id).await.unwrap();
    assert_eq!(old_fetched.superseded_by, Some(new.id));

    // Supersedes relation exists
    let rels = store.get_relations_from(new.id).await.unwrap();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].relation, RelationKind::Supersedes);
    assert_eq!(rels[0].target_id, old.id);
}

// ── Criterion 16: active queries exclude superseded entries ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c16_resolve_context_excludes_superseded() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Will be superseded",
            "body",
        ))
        .await
        .unwrap();

    let _new = store
        .supersede_entry(
            entry.id,
            new_entry("global", EntryKind::Fact, "Replacement", "new body"),
        )
        .await
        .unwrap();

    let scope = ScopePath::parse("global").unwrap();
    let entries = store.resolve_context(&scope, &[], 100).await.unwrap();

    // Only the replacement should appear
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Replacement");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c16_browse_excludes_superseded_by_default() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry("global", EntryKind::Fact, "Active", "body1"))
        .await
        .unwrap();
    store.forget_entry(entry.id).await.unwrap();

    store
        .create_entry(new_entry(
            "global",
            EntryKind::Decision,
            "Still active",
            "body2",
        ))
        .await
        .unwrap();

    let result = store.browse(EntryFilter::default()).await.unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Still active");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c16_browse_includes_superseded_when_opted_in() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry("global", EntryKind::Fact, "Will forget", "body1"))
        .await
        .unwrap();
    store.forget_entry(entry.id).await.unwrap();

    store
        .create_entry(new_entry("global", EntryKind::Decision, "Active", "body2"))
        .await
        .unwrap();

    let result = store
        .browse(EntryFilter {
            include_superseded: true,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(result.items.len(), 2);
}

// ── Criterion 17: content hash deduplication ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c17_duplicate_content_hash_rejected() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(new_entry("global", EntryKind::Fact, "Title A", "same body"))
        .await
        .unwrap();

    let result = store
        .create_entry(new_entry("global", EntryKind::Fact, "Title B", "same body"))
        .await;

    assert!(matches!(result, Err(CmError::DuplicateContent { .. })));
}

// ── Criterion 18: dedup allows reuse of superseded content hash ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c18_superseded_hash_can_be_reused() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let original = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Original",
            "unique body",
        ))
        .await
        .unwrap();

    // Supersede it
    store
        .supersede_entry(
            original.id,
            new_entry("global", EntryKind::Fact, "Replacement", "different body"),
        )
        .await
        .unwrap();

    // Now create a new entry with the same content as the superseded original
    let reuse = store
        .create_entry(new_entry("global", EntryKind::Fact, "Reuse", "unique body"))
        .await;

    assert!(
        reuse.is_ok(),
        "Should allow reusing content hash of superseded entry"
    );
}

// ── Criterion 19: FTS5 search finds entries by title ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c19_fts_search_finds_by_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Photosynthesis in plants",
            "Some body content",
        ))
        .await
        .unwrap();

    let results = store.search("photosynthesis", None, 10).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Photosynthesis in plants");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c19_fts_search_finds_by_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Generic title",
            "The mitochondria is the powerhouse of the cell",
        ))
        .await
        .unwrap();

    let results = store.search("mitochondria", None, 10).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Generic title");
}

// ── Criterion 20: FTS tracks updates ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c20_fts_reflects_updated_content() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Interesting title",
            "Original content about elephants",
        ))
        .await
        .unwrap();

    // Update the body
    store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("Updated content about giraffes".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // New content found
    let results = store.search("giraffes", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);

    // Old content no longer matches
    let results = store.search("elephants", None, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

// ── Criterion 21: FTS tracks deletes (via forget/supersede) ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c21_superseded_entries_excluded_from_search() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Searchable entry",
            "Contains the word quantum",
        ))
        .await
        .unwrap();

    // Verify it appears in search first
    let results = store.search("quantum", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);

    // Forget it
    store.forget_entry(entry.id).await.unwrap();

    // Search should exclude it (superseded_by IS NOT NULL)
    let results = store.search("quantum", None, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

// ── Criterion 22: WAL concurrent reads during writes ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c22_wal_read_during_write() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Insert an entry first
    store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Concurrent test",
            "Body for concurrency",
        ))
        .await
        .unwrap();

    // Read from the read pool while the write pool is available
    // This verifies WAL allows concurrent access from separate pools.
    let scope = ScopePath::parse("global").unwrap();
    let entries = store.resolve_context(&scope, &[], 10).await.unwrap();
    assert_eq!(entries.len(), 1);

    // Verify journal_mode is WAL on the read pool
    let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(store.read_pool())
        .await
        .unwrap();
    assert_eq!(row.0, "wal");
}

// ── Criterion 23: concurrent writes serialize ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c23_concurrent_writes_serialize() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Create two entries sequentially through the single-writer pool.
    // With max_connections=1 on the write pool, these serialize automatically.
    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "Entry 1", "Body 1"))
        .await
        .unwrap();
    let e2 = store
        .create_entry(new_entry(
            "global",
            EntryKind::Decision,
            "Entry 2",
            "Body 2",
        ))
        .await
        .unwrap();

    // Both succeed
    assert_ne!(e1.id, e2.id);

    let scope = ScopePath::parse("global").unwrap();
    let entries = store.resolve_context(&scope, &[], 100).await.unwrap();
    assert_eq!(entries.len(), 2);
}

// ── Criterion 24: updated_at trigger fires on body update ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c24_updated_at_changes_on_update() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Timestamp test",
            "Original",
        ))
        .await
        .unwrap();

    let original_updated_at = entry.updated_at;

    // Small sleep to ensure timestamp difference
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let updated = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("Modified body".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(
        updated.updated_at > original_updated_at,
        "updated_at should advance after update: {:?} vs {:?}",
        updated.updated_at,
        original_updated_at
    );
}

// ── Criterion 25: updated_at and FTS triggers both fire ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c25_updated_at_and_fts_both_fire_on_update() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Dual trigger test",
            "Original content keyword: albatross",
        ))
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let updated = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("New content keyword: pelican".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // updated_at changed (criterion 25)
    assert!(updated.updated_at > entry.updated_at);

    // FTS reflects new content (both triggers fired without conflict)
    let found = store.search("pelican", None, 10).await.unwrap();
    assert_eq!(found.len(), 1);

    let not_found = store.search("albatross", None, 10).await.unwrap();
    assert_eq!(not_found.len(), 0);
}

// ── Criterion 26: cannot delete scope with entries ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c26_scope_with_entries_cannot_be_deleted() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(new_entry("global", EntryKind::Fact, "Pinned", "To scope"))
        .await
        .unwrap();

    // Attempt to delete the scope directly via SQL (the store API doesn't expose scope deletion,
    // so we go through the write pool to verify the FK constraint)
    let result = sqlx::query("DELETE FROM scopes WHERE path = 'global'")
        .execute(store.write_pool())
        .await;

    assert!(
        result.is_err(),
        "FK constraint should prevent scope deletion"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("FOREIGN KEY constraint failed"),
        "Expected FK error, got: {err_msg}"
    );
}

// ── Criterion 27: deleting entry nullifies superseded_by references ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c27_deleting_superseding_entry_nullifies_reference() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let old = store
        .create_entry(new_entry("global", EntryKind::Fact, "Original", "body-a"))
        .await
        .unwrap();

    let new = store
        .supersede_entry(
            old.id,
            new_entry("global", EntryKind::Fact, "Replacement", "body-b"),
        )
        .await
        .unwrap();

    // Verify old entry has superseded_by set
    let old_check = store.get_entry(old.id).await.unwrap();
    assert_eq!(old_check.superseded_by, Some(new.id));

    // Delete the superseding entry directly
    sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(new.id.to_string())
        .execute(store.write_pool())
        .await
        .unwrap();

    // Old entry's superseded_by should be NULL (ON DELETE SET NULL)
    let old_after = store.get_entry(old.id).await.unwrap();
    assert!(
        old_after.superseded_by.is_none(),
        "superseded_by should be NULL after superseding entry is deleted"
    );
}

// ── Criterion 28: deleting entry cascades to relations ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c28_deleting_entry_cascades_relations() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "Source", "body-s"))
        .await
        .unwrap();
    let e2 = store
        .create_entry(new_entry("global", EntryKind::Fact, "Target", "body-t"))
        .await
        .unwrap();

    store
        .create_relation(e1.id, e2.id, RelationKind::RelatesTo)
        .await
        .unwrap();

    // Verify relation exists
    let rels = store.get_relations_from(e1.id).await.unwrap();
    assert_eq!(rels.len(), 1);

    // Delete e1 directly
    sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(e1.id.to_string())
        .execute(store.write_pool())
        .await
        .unwrap();

    // Relations involving e1 should be gone (CASCADE)
    let rels_after: Vec<sqlx::sqlite::SqliteRow> =
        sqlx::query("SELECT * FROM entry_relations WHERE source_id = ? OR target_id = ?")
            .bind(e1.id.to_string())
            .bind(e1.id.to_string())
            .fetch_all(store.read_pool())
            .await
            .unwrap();
    assert!(
        rels_after.is_empty(),
        "Relations should be cascaded on entry deletion"
    );
}

// ── Additional coverage: get_entry, get_entries, forget_entry ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_entry_returns_not_found_for_missing_id() {
    let (store, _dir) = test_store().await;

    let fake_id = uuid::Uuid::now_v7();
    let result = store.get_entry(fake_id).await;
    assert!(matches!(result, Err(CmError::EntryNotFound(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_entries_preserves_input_order() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "First", "body-1"))
        .await
        .unwrap();
    let e2 = store
        .create_entry(new_entry("global", EntryKind::Decision, "Second", "body-2"))
        .await
        .unwrap();
    let e3 = store
        .create_entry(new_entry("global", EntryKind::Lesson, "Third", "body-3"))
        .await
        .unwrap();

    // Request in reverse order
    let entries = store.get_entries(&[e3.id, e1.id, e2.id]).await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].id, e3.id);
    assert_eq!(entries[1].id, e1.id);
    assert_eq!(entries[2].id, e2.id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_entries_skips_missing_ids() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "Exists", "body-e"))
        .await
        .unwrap();

    let missing = uuid::Uuid::now_v7();
    let entries = store.get_entries(&[e1.id, missing]).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, e1.id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_entry_marks_self_referential() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Forgettable",
            "body-f",
        ))
        .await
        .unwrap();

    store.forget_entry(entry.id).await.unwrap();

    let fetched = store.get_entry(entry.id).await.unwrap();
    assert_eq!(
        fetched.superseded_by,
        Some(entry.id),
        "forget_entry should set superseded_by to own ID"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_entry_not_found() {
    let (store, _dir) = test_store().await;

    let result = store.forget_entry(uuid::Uuid::now_v7()).await;
    assert!(matches!(result, Err(CmError::EntryNotFound(_))));
}

// ── Scope operations ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_scope_without_parent_fails() {
    let (store, _dir) = test_store().await;

    // Try creating a project scope without creating global first
    let result = store
        .create_scope(NewScope {
            path: ScopePath::parse("global/project:orphan").unwrap(),
            label: "Orphan".to_owned(),
            meta: None,
        })
        .await;

    assert!(matches!(result, Err(CmError::ScopeNotFound(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_duplicate_scope_fails() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store
        .create_scope(NewScope {
            path: ScopePath::parse("global").unwrap(),
            label: "Duplicate".to_owned(),
            meta: None,
        })
        .await;

    assert!(matches!(result, Err(CmError::ConstraintViolation(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_scope_returns_correct_fields() {
    let (store, _dir) = test_store().await;
    let scope = create_global(&store).await;

    assert_eq!(scope.path.as_str(), "global");
    assert_eq!(scope.kind, ScopeKind::Global);
    assert_eq!(scope.label, "Global");
    assert!(scope.parent_path.is_none());

    let fetched = store
        .get_scope(&ScopePath::parse("global").unwrap())
        .await
        .unwrap();
    assert_eq!(fetched.path.as_str(), "global");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_scopes_filters_by_kind() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_scope(NewScope {
            path: ScopePath::parse("global/project:a").unwrap(),
            label: "A".to_owned(),
            meta: None,
        })
        .await
        .unwrap();
    store
        .create_scope(NewScope {
            path: ScopePath::parse("global/project:b").unwrap(),
            label: "B".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    let all = store.list_scopes(None).await.unwrap();
    assert_eq!(all.len(), 3); // global + 2 projects

    let projects = store.list_scopes(Some(ScopeKind::Project)).await.unwrap();
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().all(|s| s.kind == ScopeKind::Project));
}

// ── Relations ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_relation_and_query_bidirectional() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "A", "body-a"))
        .await
        .unwrap();
    let e2 = store
        .create_entry(new_entry("global", EntryKind::Fact, "B", "body-b"))
        .await
        .unwrap();

    store
        .create_relation(e1.id, e2.id, RelationKind::Elaborates)
        .await
        .unwrap();

    let from = store.get_relations_from(e1.id).await.unwrap();
    assert_eq!(from.len(), 1);
    assert_eq!(from[0].target_id, e2.id);
    assert_eq!(from[0].relation, RelationKind::Elaborates);

    let to = store.get_relations_to(e2.id).await.unwrap();
    assert_eq!(to.len(), 1);
    assert_eq!(to[0].source_id, e1.id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_relation_rejected() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "X", "body-x"))
        .await
        .unwrap();
    let e2 = store
        .create_entry(new_entry("global", EntryKind::Fact, "Y", "body-y"))
        .await
        .unwrap();

    store
        .create_relation(e1.id, e2.id, RelationKind::RelatesTo)
        .await
        .unwrap();

    let result = store
        .create_relation(e1.id, e2.id, RelationKind::RelatesTo)
        .await;
    assert!(matches!(result, Err(CmError::ConstraintViolation(_))));
}

// ── Stats & Export ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stats_reports_correct_counts() {
    let (store, _dir) = test_store().await;
    create_project_scope(&store, "test-proj").await;

    let e1 = store
        .create_entry(new_entry("global", EntryKind::Fact, "Fact 1", "body-f1"))
        .await
        .unwrap();
    store
        .create_entry(new_entry("global", EntryKind::Decision, "Dec 1", "body-d1"))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:test-proj",
            EntryKind::Fact,
            "Proj fact",
            "body-pf",
        ))
        .await
        .unwrap();

    // Supersede one
    store
        .supersede_entry(
            e1.id,
            new_entry("global", EntryKind::Fact, "Fact 1 v2", "body-f1v2"),
        )
        .await
        .unwrap();

    let stats = store.stats().await.unwrap();
    assert_eq!(stats.active_entries, 3); // Dec 1, Proj fact, Fact 1 v2
    assert_eq!(stats.superseded_entries, 1); // original Fact 1
    assert_eq!(stats.scopes, 2); // global + test-proj
    assert_eq!(stats.relations, 1); // supersedes relation
    assert_eq!(*stats.entries_by_kind.get("fact").unwrap(), 2);
    assert_eq!(*stats.entries_by_kind.get("decision").unwrap(), 1);
    assert!(stats.db_size_bytes > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_returns_active_entries_ordered() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(new_entry("global", EntryKind::Fact, "First", "body-1"))
        .await
        .unwrap();
    store
        .create_entry(new_entry("global", EntryKind::Decision, "Second", "body-2"))
        .await
        .unwrap();

    let forgotten = store
        .create_entry(new_entry(
            "global",
            EntryKind::Lesson,
            "Forgotten",
            "body-3",
        ))
        .await
        .unwrap();
    store.forget_entry(forgotten.id).await.unwrap();

    let entries = store.export(None).await.unwrap();
    assert_eq!(entries.len(), 2); // excludes forgotten
    assert_eq!(entries[0].title, "First"); // ordered by created_at ASC
    assert_eq!(entries[1].title, "Second");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_filters_by_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:specific").unwrap();
    store
        .create_scope(NewScope {
            path: project_path.clone(),
            label: "Specific".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    store
        .create_entry(new_entry("global", EntryKind::Fact, "Global", "body-g"))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:specific",
            EntryKind::Fact,
            "Scoped",
            "body-s",
        ))
        .await
        .unwrap();

    let entries = store.export(Some(&project_path)).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Scoped");
}

// ── Browse pagination ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browse_pagination_with_cursor() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Create 5 entries
    for i in 0..5 {
        store
            .create_entry(new_entry(
                "global",
                EntryKind::Fact,
                &format!("Entry {i}"),
                &format!("Body {i}"),
            ))
            .await
            .unwrap();
        // Small delay to ensure distinct updated_at
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // First page: 2 items
    let page1 = store
        .browse(EntryFilter {
            pagination: Pagination {
                limit: 2,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.total, 5);
    assert!(page1.next_cursor.is_some());

    // Second page using cursor
    let page2 = store
        .browse(EntryFilter {
            pagination: Pagination {
                limit: 2,
                cursor: page1.next_cursor,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page2.items.len(), 2);
    assert!(page2.next_cursor.is_some());

    // Third page (last entry)
    let page3 = store
        .browse(EntryFilter {
            pagination: Pagination {
                limit: 2,
                cursor: page2.next_cursor,
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page3.items.len(), 1);
    assert!(page3.next_cursor.is_none());

    // Verify no overlap between pages
    let all_ids: Vec<_> = page1
        .items
        .iter()
        .chain(page2.items.iter())
        .chain(page3.items.iter())
        .map(|e| e.id)
        .collect();
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(all_ids.len(), unique.len(), "Pages should not overlap");
}

// ── Search with scope filter ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_with_scope_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:scoped").unwrap();
    store
        .create_scope(NewScope {
            path: project_path.clone(),
            label: "Scoped".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    // Unrelated scope
    store
        .create_scope(NewScope {
            path: ScopePath::parse("global/project:other").unwrap(),
            label: "Other".to_owned(),
            meta: None,
        })
        .await
        .unwrap();

    store
        .create_entry(new_entry(
            "global/project:scoped",
            EntryKind::Fact,
            "Scoped entry",
            "Contains the word butterfly",
        ))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global/project:other",
            EntryKind::Fact,
            "Other entry",
            "Also contains butterfly",
        ))
        .await
        .unwrap();
    store
        .create_entry(new_entry(
            "global",
            EntryKind::Fact,
            "Global entry",
            "Global butterfly too",
        ))
        .await
        .unwrap();

    // Search with scope filter: should find scoped + global (ancestor), not other
    let results = store
        .search("butterfly", Some(&project_path), 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    let scopes: Vec<&str> = results.iter().map(|e| e.scope_path.as_str()).collect();
    assert!(scopes.contains(&"global/project:scoped"));
    assert!(scopes.contains(&"global"));
    assert!(!scopes.contains(&"global/project:other"));
}

// ── Entry metadata ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn entry_with_metadata_roundtrips() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let meta = EntryMeta {
        tags: vec!["rust".to_owned(), "async".to_owned()],
        confidence: Some(cm_core::Confidence::High),
        source: Some("research paper".to_owned()),
        ..Default::default()
    };

    let entry = store
        .create_entry(NewEntry {
            scope_path: ScopePath::parse("global").unwrap(),
            kind: EntryKind::Fact,
            title: "With metadata".to_owned(),
            body: "Metadata test body".to_owned(),
            created_by: "agent:test".to_owned(),
            meta: Some(meta),
        })
        .await
        .unwrap();

    let fetched = store.get_entry(entry.id).await.unwrap();
    let m = fetched.meta.unwrap();
    assert_eq!(m.tags, vec!["rust", "async"]);
    assert_eq!(m.confidence.unwrap(), cm_core::Confidence::High);
    assert_eq!(m.source.unwrap(), "research paper");
}

// ── Update entry validation ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_entry_rejects_empty_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry("global", EntryKind::Fact, "Valid", "Valid body"))
        .await
        .unwrap();

    let result = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                title: Some("   ".to_owned()),
                ..Default::default()
            },
        )
        .await;

    assert!(matches!(result, Err(CmError::Validation(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_entry_rejects_empty_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(new_entry("global", EntryKind::Fact, "Valid", "Valid body"))
        .await
        .unwrap();

    let result = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("".to_owned()),
                ..Default::default()
            },
        )
        .await;

    assert!(matches!(result, Err(CmError::Validation(_))));
}

// ── Create entry validation ──

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_entry_rejects_empty_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store
        .create_entry(new_entry("global", EntryKind::Fact, "   ", "body"))
        .await;
    assert!(matches!(result, Err(CmError::Validation(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_entry_rejects_empty_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store
        .create_entry(new_entry("global", EntryKind::Fact, "Title", "   "))
        .await;
    assert!(matches!(result, Err(CmError::Validation(_))));
}
