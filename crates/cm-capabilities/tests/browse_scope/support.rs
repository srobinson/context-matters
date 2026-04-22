use crate::common::ensure_scope;

use cm_core::{
    ContextStore, Entry, EntryKind, EntryMeta, MutationSource, NewEntry, ScopePath, WriteContext,
};
use cm_store::CmStore;

pub(crate) use crate::common::test_store;

pub(crate) fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

pub(crate) async fn seed_scoped(
    store: &CmStore,
    title: &str,
    kind: EntryKind,
    scope: &str,
) -> Entry {
    seed_scoped_with_details(store, title, kind, scope, "test:seed", &[]).await
}

pub(crate) async fn seed_scoped_with_details(
    store: &CmStore,
    title: &str,
    kind: EntryKind,
    scope: &str,
    created_by: &str,
    tags: &[&str],
) -> Entry {
    ensure_scope(store, scope).await;
    let meta = if tags.is_empty() {
        None
    } else {
        Some(EntryMeta {
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            ..Default::default()
        })
    };

    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::parse(scope).unwrap(),
                kind,
                title: title.to_owned(),
                body: format!("Body for {title}."),
                created_by: created_by.to_owned(),
                meta,
            },
            &wctx(),
        )
        .await
        .unwrap()
}
