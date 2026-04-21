//! Regression tests for recall scope ordering.

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
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

async fn ensure_scope(store: &CmStore, path: &str) {
    let sp = ScopePath::parse(path).unwrap();
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

#[tokio::test(flavor = "multi_thread")]
async fn recall_rows_sort_by_scope_depth_not_path_length() {
    let (store, _dir) = test_store().await;
    seed_entry_with_scope(
        &store,
        "Long shallow scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global/project:very-long-project-name",
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Short deeper scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global/project:a/repo:b",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("needle".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].entry.title, "Short deeper scope");
    assert_eq!(result.entries[1].entry.title, "Long shallow scope");
}
