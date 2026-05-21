use super::support::{seed_scoped, test_store};

use cm_capabilities::browse::{BrowseRequest, browse};
use cm_capabilities::scope::{CWD_INFERRED_SCOPE, ScopeResolutionConfidence, ScopeSelector};
use cm_core::{EntryKind, ScopeInferenceStrategy, ScopePath};
use cm_store::CmStore;
use std::{fs, path::Path, process::Command};

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_git_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "Test User"]);
    fs::write(root.join("README.md"), "fixture\n").unwrap();
    run_git(root, &["add", "README.md"]);
    run_git(root, &["commit", "-q", "-m", "fixture"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_cwd_inferred_resolves_repo_from_cwd() {
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
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Repo fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(resolution.requested_scope, CWD_INFERRED_SCOPE);
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
async fn browse_scope_custom_inference_rejects_cwd_inferred() {
    let (store, _dir) = test_store().await;
    let store = CmStore::new_with_scope_inference_strategy(
        store.write_pool().clone(),
        store.read_pool().clone(),
        ScopeInferenceStrategy::Custom,
    );
    seed_scoped(
        &store,
        "Repo fact",
        EntryKind::Fact,
        "global/project:helioy/repo:context-matters",
    )
    .await;

    let err = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("scope='cwd_inferred' is disabled"));
    assert!(message.contains("scope_inference.strategy='custom'"));
    assert!(message.contains("pass scope explicitly"));
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_cwd_inferred_prefers_parent_project_repo_match_over_orphan_match() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Orphan repo fact",
        EntryKind::Fact,
        "global/project:context-matters/repo:context-matters",
    )
    .await;
    seed_scoped(
        &store,
        "Canonical repo fact",
        EntryKind::Fact,
        "global/project:helioy/repo:context-matters",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/Users/alphab/Dev/LLM/DEV/helioy/context-matters".into(),
            ))),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].title, "Canonical repo fact");
    let resolution = result.resolution.as_ref().unwrap();
    assert_eq!(
        resolution.resolved_scope,
        ScopePath::parse("global/project:helioy/repo:context-matters").unwrap()
    );
    assert_eq!(resolution.confidence, ScopeResolutionConfidence::High);
    assert!(
        !resolution
            .signals
            .iter()
            .any(|signal| signal.starts_with("ambiguous scope resolution")),
        "resolution should be unambiguous: {:?}",
        resolution.signals
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_cwd_inferred_resolves_normal_git_repo_from_repo_root_identity() {
    let fixture = tempfile::tempdir().unwrap();
    let source_repo = fixture.path().join("helioy").join("context-matters");
    let nested_cwd = source_repo.join("crates").join("cm-capabilities");
    create_git_repo(&source_repo);
    fs::create_dir_all(&nested_cwd).unwrap();

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
            scope: Some(ScopeSelector::cwd_inferred(Some(nested_cwd))),
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
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_cwd_inferred_resolves_linked_worktree_from_source_repo_identity() {
    let fixture = tempfile::tempdir().unwrap();
    let source_repo = fixture.path().join("helioy").join("context-matters");
    let linked_worktree = fixture
        .path()
        .join("context-matters-worktrees")
        .join("nancy-ALP-2054");
    create_git_repo(&source_repo);
    fs::create_dir_all(linked_worktree.parent().unwrap()).unwrap();
    run_git(
        &source_repo,
        &[
            "worktree",
            "add",
            "--detach",
            linked_worktree.to_str().unwrap(),
            "HEAD",
        ],
    );

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
            scope: Some(ScopeSelector::cwd_inferred(Some(linked_worktree))),
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
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_cwd_inferred_rejects_empty_cwd() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;

    let err = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some("".into()))),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("cwd cannot be empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_cwd_inferred_resolves_project_when_repo_scope_absent() {
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
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
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
async fn browse_scope_cwd_inferred_resolves_project_from_cwd_basename() {
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
            scope: Some(ScopeSelector::cwd_inferred(Some("/tmp/helioy".into()))),
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
async fn browse_scope_cwd_inferred_ignores_non_local_project_ancestor() {
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
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/worktrees/context-matters".into(),
            ))),
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
async fn browse_scope_cwd_inferred_signals_ambiguous_repo_basename_match() {
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
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/worktrees/context-matters".into(),
            ))),
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
async fn browse_scope_cwd_inferred_falls_back_to_global_without_local_match() {
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
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/acme/no-local-match".into(),
            ))),
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
async fn browse_scope_cwd_inferred_accepts_omitted_cwd() {
    let (store, _dir) = test_store().await;
    seed_scoped(&store, "Global fact", EntryKind::Fact, "global").await;
    seed_scoped(
        &store,
        "Project fact",
        EntryKind::Fact,
        "global/project:process-cwd-omitted-fixture-should-not-match",
    )
    .await;

    let result = browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(None)),
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
}
