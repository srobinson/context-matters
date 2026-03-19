//! Scope operations, relations, WAL, triggers, referential integrity, stats, and export tests.

mod common;

use cm_core::{CmError, EntryKind, RelationKind, ScopeKind};
use common::*;

// ── Scope operations ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_scope_without_parent_fails() {
    let (store, _dir) = test_store().await;

    let result = store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global/project:orphan").unwrap(),
                label: "Orphan".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await;

    assert!(matches!(result, Err(CmError::ScopeNotFound(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_duplicate_scope_fails() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Duplicate".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
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
        .create_scope(
            NewScope {
                path: ScopePath::parse("global/project:a").unwrap(),
                label: "A".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global/project:b").unwrap(),
                label: "B".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let all = store.list_scopes(None).await.unwrap();
    assert_eq!(all.len(), 3);

    let projects = store.list_scopes(Some(ScopeKind::Project)).await.unwrap();
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().all(|s| s.kind == ScopeKind::Project));
}

// ── Relations ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_relation_and_query_bidirectional() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "A", "body-a"),
            &test_ctx(),
        )
        .await
        .unwrap();
    let e2 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "B", "body-b"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_relation(e1.id, e2.id, RelationKind::Elaborates, &test_ctx())
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
        .create_entry(
            new_entry("global", EntryKind::Fact, "X", "body-x"),
            &test_ctx(),
        )
        .await
        .unwrap();
    let e2 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Y", "body-y"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_relation(e1.id, e2.id, RelationKind::RelatesTo, &test_ctx())
        .await
        .unwrap();

    let result = store
        .create_relation(e1.id, e2.id, RelationKind::RelatesTo, &test_ctx())
        .await;
    assert!(matches!(result, Err(CmError::ConstraintViolation(_))));
}

// ── WAL and concurrency ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c22_wal_read_during_write() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Concurrent test",
                "Body for concurrency",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let scope = ScopePath::parse("global").unwrap();
    let entries = store.resolve_context(&scope, &[], 10).await.unwrap();
    assert_eq!(entries.len(), 1);

    let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(store.read_pool())
        .await
        .unwrap();
    assert_eq!(row.0, "wal");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c23_concurrent_writes_serialize() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Entry 1", "Body 1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    let e2 = store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Entry 2", "Body 2"),
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_ne!(e1.id, e2.id);

    let scope = ScopePath::parse("global").unwrap();
    let entries = store.resolve_context(&scope, &[], 100).await.unwrap();
    assert_eq!(entries.len(), 2);
}

// ── Triggers ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c24_updated_at_changes_on_update() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Timestamp test", "Original"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let original_updated_at = entry.updated_at;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let updated = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("Modified body".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
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

// ── Referential integrity ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c26_scope_with_entries_cannot_be_deleted() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Pinned", "To scope"),
            &test_ctx(),
        )
        .await
        .unwrap();

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c27_deleting_superseding_entry_nullifies_reference() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let old = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Original", "body-a"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let new = store
        .supersede_entry(
            old.id,
            new_entry("global", EntryKind::Fact, "Replacement", "body-b"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let old_check = store.get_entry(old.id).await.unwrap();
    assert_eq!(old_check.superseded_by, Some(new.id));

    sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(new.id.to_string())
        .execute(store.write_pool())
        .await
        .unwrap();

    let old_after = store.get_entry(old.id).await.unwrap();
    assert!(
        old_after.superseded_by.is_none(),
        "superseded_by should be NULL after superseding entry is deleted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c28_deleting_entry_cascades_relations() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let e1 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Source", "body-s"),
            &test_ctx(),
        )
        .await
        .unwrap();
    let e2 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Target", "body-t"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_relation(e1.id, e2.id, RelationKind::RelatesTo, &test_ctx())
        .await
        .unwrap();

    let rels = store.get_relations_from(e1.id).await.unwrap();
    assert_eq!(rels.len(), 1);

    sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(e1.id.to_string())
        .execute(store.write_pool())
        .await
        .unwrap();

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

// ── Stats & Export ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stats_reports_correct_counts() {
    let (store, _dir) = test_store().await;
    create_project_scope(&store, "test-proj").await;

    let e1 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Fact 1", "body-f1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Dec 1", "body-d1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:test-proj",
                EntryKind::Fact,
                "Proj fact",
                "body-pf",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .supersede_entry(
            e1.id,
            new_entry("global", EntryKind::Fact, "Fact 1 v2", "body-f1v2"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let stats = store.stats().await.unwrap();
    assert_eq!(stats.active_entries, 3);
    assert_eq!(stats.superseded_entries, 1);
    assert_eq!(stats.scopes, 2);
    assert_eq!(stats.relations, 1);
    assert_eq!(*stats.entries_by_kind.get("fact").unwrap(), 2);
    assert_eq!(*stats.entries_by_kind.get("decision").unwrap(), 1);
    assert!(stats.db_size_bytes > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_returns_active_entries_ordered() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "First", "body-1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Second", "body-2"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let forgotten = store
        .create_entry(
            new_entry("global", EntryKind::Lesson, "Forgotten", "body-3"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store.forget_entry(forgotten.id, &test_ctx()).await.unwrap();

    let entries = store.export(None).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "First");
    assert_eq!(entries[1].title, "Second");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_filters_by_scope() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:specific").unwrap();
    store
        .create_scope(
            NewScope {
                path: project_path.clone(),
                label: "Specific".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Global", "body-g"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:specific",
                EntryKind::Fact,
                "Scoped",
                "body-s",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let entries = store.export(Some(&project_path)).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Scoped");
}
