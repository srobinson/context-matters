//! Capability-level tests for get validation, canonicalization, and store access.

use cm_capabilities::get::{GetRequest, get};
use cm_core::{
    ContextStore, EntryKind, MutationSource, NewEntry, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};

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
                path: ScopePath::global(),
                label: "Global".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_entry(store: &CmStore, title: &str) -> cm_core::Entry {
    create_global(store).await;
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind: EntryKind::Fact,
                title: title.to_owned(),
                body: format!("Body for {title}."),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn get_returns_found_entries_with_canonical_requested_ids() {
    let (store, _dir) = test_store().await;
    let entry = seed_entry(&store, "Canonical get").await;
    let uppercase_simple_id = entry.id.simple().to_string().to_uppercase();

    let result = get(
        &store,
        GetRequest {
            ids: vec![uppercase_simple_id],
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].id, entry.id);
    assert_eq!(result.requested_ids, vec![entry.id.to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_omits_missing_entries_but_preserves_requested_ids() {
    let (store, _dir) = test_store().await;
    let missing = "019d8a01-0000-7000-8000-000000000001".to_owned();

    let result = get(
        &store,
        GetRequest {
            ids: vec![missing.clone()],
        },
    )
    .await
    .unwrap();

    assert!(result.entries.is_empty());
    assert_eq!(result.requested_ids, vec![missing]);
}
