//! Capability-level tests for stats aggregation, scope tree, and tag sorting.

use cm_capabilities::stats::{StatsRequest, TagSort, stats};
use cm_core::{
    ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};

// ── Helpers ──────────────────────────────────────────────────────

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

async fn create_scopes(store: &CmStore) {
    for (path, label) in [
        ("global", "Global"),
        ("global/project:helioy", "project:helioy"),
        ("global/project:helioy/repo:cm", "repo:cm"),
    ] {
        store
            .create_scope(
                NewScope {
                    path: ScopePath::parse(path).unwrap(),
                    label: label.to_owned(),
                    meta: None,
                },
                &wctx(),
            )
            .await
            .unwrap();
    }
}

async fn seed_tagged_entries(store: &CmStore) {
    create_scopes(store).await;

    let entries = vec![
        ("Alpha", "fact", "global", vec!["infra", "database"]),
        ("Beta", "decision", "global/project:helioy", vec!["infra"]),
        (
            "Gamma",
            "lesson",
            "global/project:helioy/repo:cm",
            vec!["architecture"],
        ),
        (
            "Delta",
            "fact",
            "global/project:helioy",
            vec!["architecture", "database"],
        ),
    ];

    for (title, kind, scope, tags) in entries {
        store
            .create_entry(
                NewEntry {
                    scope_path: ScopePath::parse(scope).unwrap(),
                    kind: kind.parse::<EntryKind>().unwrap(),
                    title: title.to_owned(),
                    body: format!("Body for {title}."),
                    created_by: "agent:test".to_owned(),
                    meta: Some(EntryMeta {
                        tags: tags.into_iter().map(String::from).collect(),
                        ..Default::default()
                    }),
                },
                &wctx(),
            )
            .await
            .unwrap();
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn stats_returns_base_counts() {
    let (store, _dir) = test_store().await;
    seed_tagged_entries(&store).await;

    let result = stats(&store, StatsRequest::default()).await.unwrap();

    assert_eq!(result.stats.active_entries, 4);
    assert_eq!(result.stats.superseded_entries, 0);
    assert_eq!(result.stats.scopes, 3);
    assert_eq!(*result.stats.entries_by_kind.get("fact").unwrap(), 2);
    assert_eq!(*result.stats.entries_by_kind.get("decision").unwrap(), 1);
    assert_eq!(*result.stats.entries_by_kind.get("lesson").unwrap(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn scope_tree_includes_all_scopes_with_entry_counts() {
    let (store, _dir) = test_store().await;
    seed_tagged_entries(&store).await;

    let result = stats(&store, StatsRequest::default()).await.unwrap();

    assert_eq!(result.scope_tree.len(), 3);

    let global = result
        .scope_tree
        .iter()
        .find(|n| n.path == "global")
        .unwrap();
    assert_eq!(global.entry_count, 1);
    assert_eq!(global.kind, "global");
    assert_eq!(global.label, "Global");

    let project = result
        .scope_tree
        .iter()
        .find(|n| n.path == "global/project:helioy")
        .unwrap();
    assert_eq!(project.entry_count, 2);
    assert_eq!(project.kind, "project");

    let repo = result
        .scope_tree
        .iter()
        .find(|n| n.path == "global/project:helioy/repo:cm")
        .unwrap();
    assert_eq!(repo.entry_count, 1);
    assert_eq!(repo.kind, "repo");
}

#[tokio::test(flavor = "multi_thread")]
async fn scope_tree_shows_zero_for_empty_scopes() {
    let (store, _dir) = test_store().await;
    create_scopes(&store).await;

    // No entries, just scopes
    let result = stats(&store, StatsRequest::default()).await.unwrap();

    assert_eq!(result.scope_tree.len(), 3);
    for node in &result.scope_tree {
        assert_eq!(
            node.entry_count, 0,
            "Scope {} should have 0 entries",
            node.path
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_sort_name_returns_alphabetical_order() {
    let (store, _dir) = test_store().await;
    seed_tagged_entries(&store).await;

    let result = stats(
        &store,
        StatsRequest {
            tag_sort: TagSort::Name,
        },
    )
    .await
    .unwrap();

    let tag_names: Vec<&str> = result
        .stats
        .entries_by_tag
        .iter()
        .map(|tc| tc.tag.as_str())
        .collect();

    let mut sorted = tag_names.clone();
    sorted.sort();
    assert_eq!(tag_names, sorted, "Tags must be alphabetically sorted");
    assert!(tag_names.contains(&"infra"));
    assert!(tag_names.contains(&"database"));
    assert!(tag_names.contains(&"architecture"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_sort_count_returns_descending_count_order() {
    let (store, _dir) = test_store().await;
    seed_tagged_entries(&store).await;

    let result = stats(
        &store,
        StatsRequest {
            tag_sort: TagSort::Count,
        },
    )
    .await
    .unwrap();

    let counts: Vec<u64> = result
        .stats
        .entries_by_tag
        .iter()
        .map(|tc| tc.count)
        .collect();

    // Store returns count DESC; verify monotonically non-increasing
    for window in counts.windows(2) {
        assert!(
            window[0] >= window[1],
            "Tags must be sorted by count descending: {:?}",
            result.stats.entries_by_tag
        );
    }

    // infra=2, database=2, architecture=2 (all equal in this fixture)
    // Verify all three tags present with correct counts
    for tc in &result.stats.entries_by_tag {
        assert_eq!(tc.count, 2, "Tag '{}' should appear in 2 entries", tc.tag);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stats_on_empty_store() {
    let (store, _dir) = test_store().await;

    let result = stats(&store, StatsRequest::default()).await.unwrap();

    assert_eq!(result.stats.active_entries, 0);
    assert_eq!(result.stats.superseded_entries, 0);
    assert_eq!(result.stats.scopes, 0);
    assert!(result.scope_tree.is_empty());
    assert!(result.stats.entries_by_tag.is_empty());
}
