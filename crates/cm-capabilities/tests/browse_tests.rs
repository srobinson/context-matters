//! Capability-level tests for browse filter construction and pagination.
//!
//! Tests exercise `cm_capabilities::browse::browse()` directly against a real
//! SQLite store, covering filtering, pagination, superseded entries, and limits.

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_capabilities::scope::ScopeResolutionConfidence;
use cm_core::{
    BrowseSort, ContextStore, Entry, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope,
    ScopePath, WriteContext,
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

async fn create_global(store: &CmStore) {
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

async fn seed(store: &CmStore, title: &str, body: &str, kind: EntryKind) {
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

async fn seed_with_scope(store: &CmStore, title: &str, kind: EntryKind, scope: &str) {
    ensure_scope(store, scope).await;
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::parse(scope).unwrap(),
                kind,
                title: title.to_owned(),
                body: format!("Body for {title}."),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_with_tag(store: &CmStore, title: &str, kind: EntryKind, tag: &str) {
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind,
                title: title.to_owned(),
                body: format!("Body for {title}."),
                created_by: "test:seed".to_owned(),
                meta: Some(EntryMeta {
                    tags: vec![tag.to_owned()],
                    ..Default::default()
                }),
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_with_creator(store: &CmStore, title: &str, kind: EntryKind, created_by: &str) {
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

async fn seed_scoped_with_details(
    store: &CmStore,
    title: &str,
    kind: EntryKind,
    scope: &str,
    created_by: &str,
    tags: &[&str],
) -> Entry {
    ensure_scope(store, scope).await;
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::parse(scope).unwrap(),
                kind,
                title: title.to_owned(),
                body: format!("Body for {title}."),
                created_by: created_by.to_owned(),
                meta: Some(EntryMeta {
                    tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
                    ..Default::default()
                }),
            },
            &wctx(),
        )
        .await
        .unwrap()
}

// ── Basic browsing ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_returns_all_entries_with_defaults() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Fact one", "Body one.", EntryKind::Fact).await;
    seed(&store, "Fact two", "Body two.", EntryKind::Decision).await;

    let result = browse(
        &store,
        BrowseRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.total, 2);
    assert!(!result.has_more);
    assert!(result.next_cursor.is_none());
}

// ── Scope filtering ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_scope_path() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Global fact", "At global.", EntryKind::Fact).await;
    seed_with_scope(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope_path: Some(ScopePath::parse("global/project:helioy").unwrap()),
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
async fn browse_scope_explicit_path_filters_exactly() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Global fact", "At global.", EntryKind::Fact).await;
    seed_with_scope(
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
async fn browse_scope_and_scope_path_must_not_conflict() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

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
    create_global(&store).await;
    seed(&store, "Global fact", "At global.", EntryKind::Fact).await;
    seed_with_scope(
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
            .any(|signal| { signal == "cwd basename matched repo scope segment: context-matters" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_resolves_project_when_repo_scope_absent() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Global fact", "At global.", EntryKind::Fact).await;
    seed_with_scope(
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
    create_global(&store).await;
    seed(&store, "Global fact", "At global.", EntryKind::Fact).await;
    seed_with_scope(
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
            .any(|signal| { signal == "no local scope matched cwd; using global fallback" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_preserves_other_filters_and_pagination() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
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

// ── Kind filtering ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_kind() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "A fact", "Body.", EntryKind::Fact).await;
    seed(&store, "A decision", "Body.", EntryKind::Decision).await;
    seed(&store, "A lesson", "Body.", EntryKind::Lesson).await;

    let result = browse(
        &store,
        BrowseRequest {
            kind: Some(EntryKind::Decision),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].kind, EntryKind::Decision);
}

// ── Tag filtering ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_tag() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_with_tag(&store, "Tagged entry", EntryKind::Fact, "infra").await;
    seed(&store, "Untagged entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            tag: Some("infra".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Tagged entry");
}

// ── Creator filtering ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_filters_by_created_by() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_with_creator(&store, "Agent entry", EntryKind::Fact, "agent:nancy").await;
    seed_with_creator(&store, "MCP entry", EntryKind::Fact, "mcp:claude").await;

    let result = browse(
        &store,
        BrowseRequest {
            created_by: Some("agent:nancy".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Agent entry");
}

// ── Superseded entries ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_excludes_superseded_by_default() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

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

    let result = browse(
        &store,
        BrowseRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Replacement");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_includes_superseded_when_opted_in() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

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

    let result = browse(
        &store,
        BrowseRequest {
            include_superseded: true,
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 2);
}

// ── Pagination ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_pagination_with_cursor() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..5 {
        seed(
            &store,
            &format!("Entry {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }

    // First page: limit 2
    let page1 = browse(
        &store,
        BrowseRequest {
            limit: 2,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page1.entries.len(), 2);
    assert!(page1.has_more);
    assert!(page1.next_cursor.is_some());
    assert_eq!(page1.total, 5);

    // Second page: use cursor
    let page2 = browse(
        &store,
        BrowseRequest {
            limit: 2,
            cursor: page1.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page2.entries.len(), 2);
    assert!(page2.has_more);

    // Third page: remaining entry
    let page3 = browse(
        &store,
        BrowseRequest {
            limit: 2,
            cursor: page2.next_cursor,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(page3.entries.len(), 1);
    assert!(!page3.has_more);
    assert!(page3.next_cursor.is_none());
}

// ── Limit enforcement ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_respects_limit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for i in 0..10 {
        seed(
            &store,
            &format!("Entry {i}"),
            &format!("Content {i}."),
            EntryKind::Fact,
        )
        .await;
    }

    let result = browse(
        &store,
        BrowseRequest {
            limit: 3,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 3);
    assert_eq!(result.total, 10);
    assert!(result.has_more);
}

// ── has_more reflects cursor presence ────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn has_more_false_when_all_returned() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Only entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert!(!result.has_more);
    assert!(result.next_cursor.is_none());
}

// ── sort_used observability ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_populates_sort_used_default() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Only entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Defaulted `sort` must surface as `Recent` so the formatter can render
    // the header without having to reach back into the original request.
    assert_eq!(result.sort_used, BrowseSort::Recent);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_populates_sort_used_explicit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "Only entry", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            sort: BrowseSort::Oldest,
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.sort_used, BrowseSort::Oldest);
}

// ── Empty result ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_returns_empty_when_no_matches() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed(&store, "A fact", "Body.", EntryKind::Fact).await;

    let result = browse(
        &store,
        BrowseRequest {
            kind: Some(EntryKind::Decision),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(result.entries.is_empty());
    assert_eq!(result.total, 0);
    assert!(!result.has_more);
}
