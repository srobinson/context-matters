#![allow(dead_code)]

pub use cm_core::{ContextStore, MutationSource, NewEntry, NewScope, ScopePath, WriteContext};
pub use cm_store::{CmStore, schema};

pub fn test_ctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

pub async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();

    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

pub async fn create_global(store: &CmStore) -> cm_core::Scope {
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Global".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap()
}

pub async fn create_project_scope(store: &CmStore, project: &str) -> cm_core::Scope {
    create_global(store).await;
    let path = format!("global/project:{project}");
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse(&path).unwrap(),
                label: project.to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap()
}

pub fn new_entry(scope: &str, kind: cm_core::EntryKind, title: &str, body: &str) -> NewEntry {
    NewEntry {
        scope_path: ScopePath::parse(scope).unwrap(),
        kind,
        title: title.to_owned(),
        body: body.to_owned(),
        created_by: "agent:test".to_owned(),
        meta: None,
    }
}
