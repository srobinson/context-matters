//! Supersede, forget, dedup, and active-entry filtering tests.

mod common;

use cm_core::{CmError, EntryFilter, EntryKind, RelationKind, ScopePath};
use common::*;

// ── Supersede ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c15_supersede_entry_marks_old_and_creates_new() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let old = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Old fact", "Old body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let replacement = new_entry("global", EntryKind::Fact, "New fact", "New body");
    let new = store
        .supersede_entry(old.id, replacement, &test_ctx())
        .await
        .unwrap();

    assert!(new.superseded_by.is_none());
    assert_eq!(new.title, "New fact");

    let old_fetched = store.get_entry(old.id).await.unwrap();
    assert_eq!(old_fetched.superseded_by, Some(new.id));

    let rels = store.get_relations_from(new.id).await.unwrap();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].relation, RelationKind::Supersedes);
    assert_eq!(rels[0].target_id, old.id);
}

// ── Active filtering ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c16_resolve_context_excludes_superseded() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Will be superseded", "body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let _new = store
        .supersede_entry(
            entry.id,
            new_entry("global", EntryKind::Fact, "Replacement", "new body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let scope = ScopePath::parse("global").unwrap();
    let entries = store.resolve_context(&scope, &[], 100).await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Replacement");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c16_browse_excludes_superseded_by_default() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Active", "body1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Still active", "body2"),
            &test_ctx(),
        )
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
        .create_entry(
            new_entry("global", EntryKind::Fact, "Will forget", "body1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Active", "body2"),
            &test_ctx(),
        )
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

// ── Dedup ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c17_duplicate_content_hash_rejected() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Title A", "same body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let result = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Title B", "same body"),
            &test_ctx(),
        )
        .await;

    assert!(matches!(result, Err(CmError::DuplicateContent { .. })));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c18_superseded_hash_can_be_reused() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let original = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Original", "unique body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .supersede_entry(
            original.id,
            new_entry("global", EntryKind::Fact, "Replacement", "different body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let reuse = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Reuse", "unique body"),
            &test_ctx(),
        )
        .await;

    assert!(
        reuse.is_ok(),
        "Should allow reusing content hash of superseded entry"
    );
}

// ── Forget ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_entry_marks_self_referential() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Forgettable", "body-f"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

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

    let result = store.forget_entry(uuid::Uuid::now_v7(), &test_ctx()).await;
    assert!(matches!(result, Err(CmError::EntryNotFound(_))));
}
