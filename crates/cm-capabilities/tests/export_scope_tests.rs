//! Capability level tests for export scope selection.

mod common;

use cm_capabilities::export::{ExportRequest, export};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{EntryKind, ScopePath};
use common::{seed_entry_with_scope, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn export_filters_entries_by_exact_scope_selector_path() {
    let (store, _dir) = test_store().await;
    seed_entry_with_scope(&store, "Global", "Global body.", EntryKind::Fact, "global").await;
    seed_entry_with_scope(
        &store,
        "Project",
        "Project body.",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;
    seed_entry_with_scope(
        &store,
        "Repo",
        "Repo body.",
        EntryKind::Fact,
        "global/project:helioy/repo:context-matters",
    )
    .await;

    let view = export(
        &store,
        ExportRequest {
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
            format: "json".to_owned(),
        },
    )
    .await
    .unwrap();

    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].title, "Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn export_filters_entries_by_cwd_inferred_selector() {
    let (store, _dir) = test_store().await;
    seed_entry_with_scope(&store, "Global", "Global body.", EntryKind::Fact, "global").await;
    seed_entry_with_scope(
        &store,
        "Repo",
        "Repo body.",
        EntryKind::Fact,
        "global/project:helioy/repo:context-matters",
    )
    .await;

    let view = export(
        &store,
        ExportRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            format: "json".to_owned(),
        },
    )
    .await
    .unwrap();

    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].title, "Repo");
}
