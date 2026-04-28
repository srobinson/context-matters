use std::path::PathBuf;

use cm_capabilities::scope::{
    BrowseScopeMode, ResolvedScopeSelection, ScopeResolution, ScopeResolutionCandidate,
    ScopeResolutionConfidence, ScopeSelector,
};
use cm_core::ScopePath;

fn repo_scope() -> ScopePath {
    ScopePath::parse("global/project:helioy/repo:context-matters").unwrap()
}

fn resolution(
    confidence: ScopeResolutionConfidence,
    candidates: Vec<ScopeResolutionCandidate>,
) -> ScopeResolution {
    ScopeResolution {
        requested_scope: "cwd_inferred".to_owned(),
        resolved_scope: repo_scope(),
        scope_mode: BrowseScopeMode::Resolved,
        confidence,
        candidates,
        signals: vec![],
    }
}

fn candidate(scope: ScopePath, score: i32) -> ScopeResolutionCandidate {
    ScopeResolutionCandidate {
        scope,
        score,
        matched: vec!["repo".to_owned()],
    }
}

#[test]
fn scope_selector_parses_exact_path() {
    let scope = repo_scope();

    let selector = ScopeSelector::parse(scope.as_str()).unwrap();

    assert_eq!(selector, ScopeSelector::Path(scope));
}

#[test]
fn scope_selector_constructs_cwd_inferred() {
    let cwd = PathBuf::from("/tmp/helioy/context-matters");

    let parsed = ScopeSelector::parse("cwd_inferred").unwrap();
    let explicit = ScopeSelector::cwd_inferred(Some(cwd.clone()));

    assert_eq!(parsed, ScopeSelector::CwdInferred { cwd: None });
    assert_eq!(explicit, ScopeSelector::CwdInferred { cwd: Some(cwd) });
}

#[test]
fn scope_selector_rejects_removed_auto_value() {
    let err = ScopeSelector::parse("auto").unwrap_err();

    assert!(err.to_string().contains("cwd_inferred"));
}

#[test]
fn scope_selector_rejects_empty_input() {
    let err = ScopeSelector::parse("   ").unwrap_err();

    assert!(err.to_string().contains("empty"));
}

#[test]
fn scope_selector_requested_scope_matches_variant() {
    let scope = repo_scope();
    let exact = ScopeSelector::Path(scope.clone());
    let inferred = ScopeSelector::cwd_inferred(None);

    assert_eq!(exact.requested_scope(), scope.as_str());
    assert_eq!(inferred.requested_scope(), "cwd_inferred");
}

#[test]
fn scope_selector_optional_scope_attaches_cwd_to_inferred_only() {
    let cwd = PathBuf::from("/tmp/helioy/context-matters");

    let omitted_without_cwd = ScopeSelector::from_optional_scope(None, None).unwrap();
    let inferred_without_cwd = ScopeSelector::from_optional_scope(Some("cwd_inferred"), None)
        .unwrap()
        .unwrap();
    let exact_without_cwd = ScopeSelector::from_optional_scope(Some(repo_scope().as_str()), None)
        .unwrap()
        .unwrap();
    let inferred = ScopeSelector::from_optional_scope(Some("cwd_inferred"), Some(cwd.clone()))
        .unwrap()
        .unwrap();
    let omitted = ScopeSelector::from_optional_scope(None, Some(cwd.clone()))
        .unwrap()
        .unwrap();
    let exact_err =
        ScopeSelector::from_optional_scope(Some(repo_scope().as_str()), Some(cwd.clone()))
            .unwrap_err();

    assert_eq!(omitted_without_cwd, None);
    assert_eq!(inferred_without_cwd, ScopeSelector::cwd_inferred(None));
    assert_eq!(exact_without_cwd, ScopeSelector::Path(repo_scope()));
    assert_eq!(inferred, ScopeSelector::cwd_inferred(Some(cwd.clone())));
    assert_eq!(omitted, ScopeSelector::cwd_inferred(Some(cwd)));
    assert!(exact_err.to_string().contains("cwd can only be supplied"));
}

#[test]
fn scope_selector_optional_scope_rejects_empty_cwd() {
    let err = ScopeSelector::from_optional_scope(None, Some(PathBuf::from(""))).unwrap_err();

    assert!(err.to_string().contains("cwd cannot be empty"));
}

#[test]
fn scope_selection_policy_allows_exact_write() {
    let scope = repo_scope();
    let selection = ResolvedScopeSelection {
        scope_path: Some(scope.clone()),
        resolution: None,
        requested_scope: scope.as_str().to_owned(),
    };

    assert_eq!(selection.read_scope_path().unwrap(), &scope);
    assert_eq!(selection.write_scope_path().unwrap(), &scope);
}

#[test]
fn scope_selection_policy_allows_reads_from_low_confidence_inference() {
    let scope = repo_scope();
    let selection = ResolvedScopeSelection {
        scope_path: Some(scope.clone()),
        resolution: Some(resolution(
            ScopeResolutionConfidence::Low,
            vec![candidate(scope.clone(), 30)],
        )),
        requested_scope: "cwd_inferred".to_owned(),
    };

    assert_eq!(selection.read_scope_path().unwrap(), &scope);
}

#[test]
fn scope_selection_policy_allows_unique_high_confidence_write() {
    let scope = repo_scope();
    let selection = ResolvedScopeSelection {
        scope_path: Some(scope.clone()),
        resolution: Some(resolution(
            ScopeResolutionConfidence::High,
            vec![candidate(scope.clone(), 230)],
        )),
        requested_scope: "cwd_inferred".to_owned(),
    };

    assert_eq!(selection.write_scope_path().unwrap(), &scope);
}

#[test]
fn scope_selection_policy_rejects_low_confidence_write() {
    let scope = repo_scope();
    let selection = ResolvedScopeSelection {
        scope_path: Some(scope.clone()),
        resolution: Some(resolution(
            ScopeResolutionConfidence::Low,
            vec![candidate(scope, 30)],
        )),
        requested_scope: "cwd_inferred".to_owned(),
    };

    let err = selection.write_scope_path().unwrap_err();

    assert!(err.to_string().contains("high confidence"));
}

#[test]
fn scope_selection_policy_rejects_unresolved_read_and_write() {
    let selection = ResolvedScopeSelection {
        scope_path: None,
        resolution: None,
        requested_scope: "cwd_inferred".to_owned(),
    };

    let read_err = selection.read_scope_path().unwrap_err();
    let write_err = selection.write_scope_path().unwrap_err();

    assert!(read_err.to_string().contains("did not resolve"));
    assert!(write_err.to_string().contains("did not resolve"));
}

#[test]
fn scope_selection_policy_rejects_high_confidence_without_candidates() {
    let scope = repo_scope();
    let selection = ResolvedScopeSelection {
        scope_path: Some(scope.clone()),
        resolution: Some(resolution(ScopeResolutionConfidence::High, vec![])),
        requested_scope: "cwd_inferred".to_owned(),
    };

    let err = selection.write_scope_path().unwrap_err();

    assert!(err.to_string().contains("unique"));
}

#[test]
fn scope_selection_policy_rejects_tied_high_confidence_write() {
    let scope = repo_scope();
    let other_scope = ScopePath::parse("global/project:helioy/repo:context-matters-api").unwrap();
    let selection = ResolvedScopeSelection {
        scope_path: Some(scope.clone()),
        resolution: Some(resolution(
            ScopeResolutionConfidence::High,
            vec![candidate(scope, 230), candidate(other_scope, 230)],
        )),
        requested_scope: "cwd_inferred".to_owned(),
    };

    let err = selection.write_scope_path().unwrap_err();

    assert!(err.to_string().contains("unique"));
}
