//! Regression tests for recall scope defaulting and ordering.

use cm_capabilities::recall::{
    RECALL_SCOPE_DEFAULT_ADVISORY, RecallAdvisory, RecallRequest, RecallRouting, recall,
};
use cm_capabilities::scope::ScopeSelector;
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
async fn recall_rows_sort_by_scope_depth_with_explicit_scope() {
    let (store, _dir) = test_store().await;
    seed_entry_with_scope(
        &store,
        "Broad global scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global",
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Narrow project scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global/project:very-long-project-name",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("needle".to_owned()),
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:very-long-project-name").unwrap(),
            )),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].entry.title, "Narrow project scope");
    assert_eq!(result.entries[1].entry.title, "Broad global scope");
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_rows_sort_by_scope_depth_with_cwd_inferred_scope() {
    let (store, _dir) = test_store().await;
    seed_entry_with_scope(
        &store,
        "Broad global scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global",
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Project scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Repo scope",
        "Depth ordering regression needle.",
        EntryKind::Fact,
        "global/project:helioy/repo:context-matters",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("needle".to_owned()),
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.entries.len(), 3);
    assert_eq!(result.entries[0].entry.title, "Repo scope");
    assert_eq!(result.entries[1].entry.title, "Project scope");
    assert_eq!(result.entries[2].entry.title, "Broad global scope");
    assert_eq!(
        result.scope_chain,
        vec![
            "global/project:helioy/repo:context-matters",
            "global/project:helioy",
            "global",
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn omitted_scope_defaults_to_global_with_advisory() {
    let (store, _dir) = test_store().await;
    seed_entry_with_scope(
        &store,
        "Global note",
        "Visible globally.",
        EntryKind::Fact,
        "global",
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Project note",
        "Project-only context.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert_eq!(result.scope_chain, vec!["global"]);
    assert_eq!(
        result.advisories,
        vec![RecallAdvisory::ScopeDefaulted {
            applied: "global".to_owned()
        }]
    );
    assert_eq!(result.advisories[0].body(), RECALL_SCOPE_DEFAULT_ADVISORY);
    assert!(
        result
            .entries
            .iter()
            .all(|row| row.entry.scope_path.as_str() == "global")
    );
}
