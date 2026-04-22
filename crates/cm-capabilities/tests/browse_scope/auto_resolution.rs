use super::support::{seed_scoped, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_capabilities::scope::ScopeResolutionConfidence;
use cm_core::{EntryKind, ScopePath};

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
            limit: Some(20),
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
            limit: Some(20),
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
async fn browse_scope_auto_resolves_project_from_cwd_basename() {
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
            cwd: Some("/tmp/helioy".into()),
            limit: Some(20),
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
    assert!(
        resolution
            .signals
            .iter()
            .any(|signal| { signal == "cwd basename matched project scope segment: helioy" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_ignores_non_local_project_ancestor() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Ancestor project fact",
        EntryKind::Fact,
        "global/project:helioy",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/helioy/worktrees/context-matters".into()),
            limit: Some(20),
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
            .candidates
            .iter()
            .all(|candidate| candidate.scope != ScopePath::parse("global/project:helioy").unwrap())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_auto_signals_ambiguous_repo_basename_match() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Alpha repo fact",
        EntryKind::Fact,
        "global/project:alpha/repo:context-matters",
    )
    .await;
    seed_scoped(
        &store,
        "Beta repo fact",
        EntryKind::Fact,
        "global/project:beta/repo:context-matters",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some("/tmp/worktrees/context-matters".into()),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Alpha repo fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(
        resolution.resolved_scope,
        ScopePath::parse("global/project:alpha/repo:context-matters").unwrap()
    );
    assert_eq!(resolution.confidence, ScopeResolutionConfidence::Medium);
    assert!(resolution.signals.iter().any(|signal| {
        signal.starts_with("ambiguous scope resolution; 2 candidates share top score")
    }));
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
            limit: Some(20),
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
async fn browse_scope_auto_uses_process_cwd_when_cwd_omitted() {
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
            limit: Some(20),
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
