pub(crate) use crate::common::{
    create_global, seed_entry, seed_entry_with_scope, seed_entry_with_tags, test_store,
};

use cm_core::{ContextStore, EntryKind, MutationSource, NewEntry, ScopePath, WriteContext};
use cm_store::CmStore;

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

pub(crate) async fn seed_with_creator(
    store: &CmStore,
    title: &str,
    kind: EntryKind,
    created_by: &str,
) {
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind,
                title: title.to_owned(),
                body: format!("Body for {title}."),
                created_by: created_by.to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

pub(crate) async fn seed_numbered_entries(store: &CmStore, count: usize) {
    for i in 0..count {
        seed_entry(
            store,
            &format!("Entry {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }
}

pub(crate) async fn seed_superseded_fact_pair(store: &CmStore) {
    let original = store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind: EntryKind::Fact,
                title: "Original".to_owned(),
                body: "Original body.".to_owned(),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();

    store
        .supersede_entry(
            original.id,
            NewEntry {
                scope_path: ScopePath::global(),
                kind: EntryKind::Fact,
                title: "Replacement".to_owned(),
                body: "Replacement body.".to_owned(),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}
