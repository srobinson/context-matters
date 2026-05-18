#![allow(dead_code)]

use cm_core::{
    CmError, ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope, ScopePath,
    WriteContext,
};
use cm_store::{CmStore, schema};

pub(crate) const CANONICAL_CONTEXT_REPO_SCOPE: &str = "global/project:helioy/repo:context-matters";
pub(crate) const ORPHAN_CONTEXT_REPO_SCOPE: &str =
    "global/project:context-matters/repo:context-matters";

pub(crate) async fn test_store() -> (CmStore, tempfile::TempDir) {
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

pub(crate) async fn create_global(store: &CmStore) {
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

pub(crate) async fn ensure_scope(store: &CmStore, path: &str) {
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

pub(crate) async fn assert_scope_missing(store: &CmStore, path: &str) {
    let scope_path = ScopePath::parse(path).unwrap();
    let result = store.get_scope(&scope_path).await;
    assert!(matches!(result, Err(CmError::ScopeNotFound(_))));
}

pub(crate) fn assert_scope_collision_error(err: CmError, requested: &str, existing: &str) {
    match err {
        CmError::Validation(msg) => {
            assert!(msg.contains("refusing to auto-create scope"));
            assert!(msg.contains(requested));
            assert!(msg.contains(existing));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

pub(crate) async fn seed_entry(store: &CmStore, title: &str, body: &str, kind: EntryKind) {
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

pub(crate) async fn seed_entry_with_scope(
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

pub(crate) async fn seed_entry_with_tags(
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

pub(crate) async fn seed_scoped_tagged_entry(
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
