//! Entry CRUD, validation, and metadata tests.

mod common;

use cm_core::{CmError, EntryKind, EntryMeta, NewEntry, ScopePath};
use common::*;

// ── Create entry ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c11_create_entry_with_valid_scope_returns_entry_with_id() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Test title", "Test body"),
            &test_ctx(),
        )
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c12_create_entry_with_missing_scope_fails() {
    let (store, _dir) = test_store().await;

    let result = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Orphan", "No scope"),
            &test_ctx(),
        )
        .await;

    assert!(matches!(result, Err(CmError::ScopeNotFound(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_entry_rejects_empty_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "   ", "body"),
            &test_ctx(),
        )
        .await;
    assert!(matches!(result, Err(CmError::Validation(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_entry_rejects_empty_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Title", "   "),
            &test_ctx(),
        )
        .await;
    assert!(matches!(result, Err(CmError::Validation(_))));
}

// ── Get entry ───────────────────────────────────────────────────

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
        .create_entry(
            new_entry("global", EntryKind::Fact, "First", "body-1"),
            &test_ctx(),
        )
        .await
        .unwrap();
    let e2 = store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Second", "body-2"),
            &test_ctx(),
        )
        .await
        .unwrap();
    let e3 = store
        .create_entry(
            new_entry("global", EntryKind::Lesson, "Third", "body-3"),
            &test_ctx(),
        )
        .await
        .unwrap();

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
        .create_entry(
            new_entry("global", EntryKind::Fact, "Exists", "body-e"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let missing = uuid::Uuid::now_v7();
    let entries = store.get_entries(&[e1.id, missing]).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, e1.id);
}

// ── Update entry validation ─────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_entry_rejects_empty_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Valid", "Valid body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let result = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                title: Some("   ".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await;

    assert!(matches!(result, Err(CmError::Validation(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_entry_rejects_empty_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Valid", "Valid body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let result = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await;

    assert!(matches!(result, Err(CmError::Validation(_))));
}

// ── Metadata ────────────────────────────────────────────────────

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
        .create_entry(
            NewEntry {
                scope_path: ScopePath::parse("global").unwrap(),
                kind: EntryKind::Fact,
                title: "With metadata".to_owned(),
                body: "Metadata test body".to_owned(),
                created_by: "agent:test".to_owned(),
                meta: Some(meta),
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let fetched = store.get_entry(entry.id).await.unwrap();
    let m = fetched.meta.unwrap();
    assert_eq!(m.tags, vec!["rust", "async"]);
    assert_eq!(m.confidence.unwrap(), cm_core::Confidence::High);
    assert_eq!(m.source.unwrap(), "research paper");
}
