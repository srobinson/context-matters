//! Capability tests for smart browse scope resolution.
//!
//! These tests stay separate from the core browse filter tests so each
//! integration test file remains focused and below the project size limit.

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_capabilities::scope::ScopeResolutionConfidence;
use cm_core::{
    BrowseSort, ContextStore, Entry, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope,
    ScopePath, WriteContext,
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

async fn seed_scoped(store: &CmStore, title: &str, kind: EntryKind, scope: &str) -> Entry {
    seed_scoped_with_details(store, title, kind, scope, "test:seed", &[]).await
}

async fn seed_scoped_with_details(
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

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_explicit_path_filters_exactly() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("global/project:helioy".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Project fact");
    assert!(result.resolution.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_matching_scope_and_scope_path_filter_exactly() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;
    let project_scope = ScopePath::parse("global/project:helioy").unwrap();

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some(project_scope.as_str().to_owned()),
            scope_path: Some(project_scope),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Project fact");
    assert!(result.resolution.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_and_scope_path_must_not_conflict() {
    let (store, _dir) = test_store().await;

    let err = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            scope_path: Some(ScopePath::parse("global").unwrap()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("cannot be combined with scope_path"),
        "unexpected error: {err}",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_resolves_repo_from_cwd() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Repo fact",
        EntryKind::Fact,
        "global/project:helioy/repo:context-matters",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/context-matters".into()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Repo fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(
        resolution.resolved_scope,
        ScopePath::parse("global/project:helioy/repo:context-matters").unwrap()
    );
    assert_eq!(resolution.confidence, ScopeResolutionConfidence::High);
    assert_eq!(
        resolution.candidates[0].scope,
        ScopePath::parse("global/project:helioy/repo:context-matters").unwrap()
    );
    assert!(
        resolution
            .signals
            .iter()
            .any(|signal| signal == "cwd basename matched repo scope segment: context-matters")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_resolves_project_when_repo_scope_absent() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/context-matters".into()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Project fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(
        resolution.resolved_scope,
        ScopePath::parse("global/project:helioy").unwrap()
    );
    assert_eq!(resolution.confidence, ScopeResolutionConfidence::Medium);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_falls_back_to_global_without_local_match() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Other project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/acme/no-local-match".into()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Global fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(resolution.resolved_scope, ScopePath::global());
    assert_eq!(resolution.confidence, ScopeResolutionConfidence::VeryLow);
    assert!(
        resolution
            .signals
            .iter()
            .any(|signal| signal == "no local scope matched cwd; using global fallback")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_falls_back_to_global_without_cwd() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Global fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(resolution.resolved_scope, ScopePath::global());
    assert_eq!(resolution.confidence, ScopeResolutionConfidence::VeryLow);
    assert!(
        resolution
            .signals
            .iter()
            .any(|signal| signal == "no cwd supplied; using global fallback")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_preserves_other_filters_and_pagination() {
    let (store, _dir) = test_store().await;
    let repo_scope = "global/project:helioy/repo:context-matters";

    let original = seed_scoped_with_details(
        &store,
        "Scoped 00",
        EntryKind::Fact,
        repo_scope,
        "agent:auto",
        &["keep"],
    )
    .await;
    store
        .supersede_entry(
            original.id,
            NewEntry {
                scope_path: ScopePath::parse(repo_scope).unwrap(),
                kind: EntryKind::Fact,
                title: "Scoped 04".to_owned(),
                body: "Replacement body.".to_owned(),
                created_by: "agent:auto".to_owned(),
                meta: Some(EntryMeta {
                    tags: vec!["keep".to_owned()],
                    ..Default::default()
                }),
            },
            &wctx(),
        )
        .await
        .unwrap();

    for title in ["Scoped 01", "Scoped 02", "Scoped 03"] {
        seed_scoped_with_details(
            &store,
            title,
            EntryKind::Fact,
            repo_scope,
            "agent:auto",
            &["keep"],
        )
        .await;
    }

    seed_scoped_with_details(
        &store,
        "Wrong kind",
        EntryKind::Decision,
        repo_scope,
        "agent:auto",
        &["keep"],
    )
    .await;
    seed_scoped_with_details(
        &store,
        "Wrong tag",
        EntryKind::Fact,
        repo_scope,
        "agent:auto",
        &["drop"],
    )
    .await;
    seed_scoped_with_details(
        &store,
        "Wrong creator",
        EntryKind::Fact,
        repo_scope,
        "agent:other",
        &["keep"],
    )
    .await;
    seed_scoped_with_details(
        &store,
        "Wrong scope",
        EntryKind::Fact,
        "global",
        "agent:auto",
        &["keep"],
    )
    .await;

    let page1 = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/context-matters".into()),
            kind: Some(EntryKind::Fact),
            tag: Some("keep".to_owned()),
            created_by: Some("agent:auto".to_owned()),
            include_superseded: true,
            sort: BrowseSort::TitleAsc,
            limit: 2,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page1.total, 5);
    assert_eq!(page1.entries.len(), 2);
    assert_eq!(page1.entries[0].title, "Scoped 00");
    assert_eq!(page1.entries[1].title, "Scoped 01");
    assert!(page1.has_more);
    assert_eq!(
        page1.resolution.as_ref().unwrap().resolved_scope,
        ScopePath::parse(repo_scope).unwrap()
    );

    let page2 = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/context-matters".into()),
            kind: Some(EntryKind::Fact),
            tag: Some("keep".to_owned()),
            created_by: Some("agent:auto".to_owned()),
            include_superseded: true,
            sort: BrowseSort::TitleAsc,
            limit: 2,
            cursor: page1.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page2.total, 5);
    assert_eq!(page2.entries.len(), 2);
    assert_eq!(page2.entries[0].title, "Scoped 02");
    assert_eq!(page2.entries[1].title, "Scoped 03");
}
