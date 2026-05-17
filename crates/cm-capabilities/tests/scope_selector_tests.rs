use std::path::PathBuf;

use cm_capabilities::scope::{
    BrowseScopeMode, ResolvedScopeSelection, ScopeResolution, ScopeResolutionCandidate,
    ScopeResolutionConfidence, ScopeSelector,
};
use cm_core::ScopePath;
use serde_json::json;

fn repo_scope() -> ScopePath {
    ScopePath::parse("global/project:helioy/repo:context-matters").unwrap()
}

fn project_scope() -> ScopePath {
    ScopePath::parse("global/project:helioy").unwrap()
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
fn scope_selector_parses_structured_path() {
    let scope = repo_scope();

    let selector = ScopeSelector::parse(
        r#"{"kind":"path","path":"global/project:helioy/repo:context-matters"}"#,
    )
    .unwrap();

    assert_eq!(selector, ScopeSelector::Path(scope));
}

#[test]
fn scope_selector_parses_plain_scope_path() {
    let selector = ScopeSelector::parse("global/project:helioy/repo:context-matters").unwrap();

    assert_eq!(selector, ScopeSelector::Path(repo_scope()));
}

#[test]
fn scope_selector_parses_project_repo_session_sugar() {
    let cases = [
        (
            json!({"kind": "project", "project": "helioy"}),
            "global/project:helioy",
        ),
        (
            json!({"kind": "repo", "project": "helioy", "repo": "context-matters"}),
            "global/project:helioy/repo:context-matters",
        ),
        (
            json!({
                "kind": "session",
                "project": "helioy",
                "repo": "context-matters",
                "session": "analysis"
            }),
            "global/project:helioy/repo:context-matters/session:analysis",
        ),
    ];

    for (input, expected) in cases {
        let selector: ScopeSelector = serde_json::from_value(input).unwrap();
        assert_eq!(
            selector,
            ScopeSelector::Path(ScopePath::parse(expected).unwrap())
        );
    }
}

#[test]
fn scope_selector_parses_descendants_alias() {
    let selector: ScopeSelector =
        serde_json::from_value(json!({"kind": "descendants", "path": "global/project:helioy"}))
            .unwrap();

    assert_eq!(selector, ScopeSelector::Subtree(project_scope()));
}

#[test]
fn scope_selector_round_trips_structured_variants() {
    let cwd = PathBuf::from("/tmp/helioy/context-matters");
    let path = ScopeSelector::Path(repo_scope());
    let inferred = ScopeSelector::cwd_inferred(Some(cwd.clone()));
    let subtree = ScopeSelector::Subtree(project_scope());
    let set = ScopeSelector::Set(vec![project_scope(), repo_scope()]);
    let all = ScopeSelector::All;

    let cases = [
        (
            path,
            json!({"kind": "path", "path": "global/project:helioy/repo:context-matters"}),
        ),
        (
            inferred,
            json!({"kind": "cwd_inferred", "cwd": "/tmp/helioy/context-matters"}),
        ),
        (
            subtree,
            json!({"kind": "subtree", "path": "global/project:helioy"}),
        ),
        (
            set,
            json!({
                "kind": "set",
                "paths": [
                    "global/project:helioy",
                    "global/project:helioy/repo:context-matters"
                ]
            }),
        ),
        (all, json!({"kind": "all"})),
    ];

    for (selector, value) in cases {
        let encoded = serde_json::to_value(&selector).unwrap();
        assert_eq!(encoded, value);

        let decoded: ScopeSelector = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, selector);
    }
}

#[test]
fn scope_selector_constructs_cwd_inferred() {
    let cwd = PathBuf::from("/tmp/helioy/context-matters");

    let parsed = ScopeSelector::parse(r#"{"kind":"cwd_inferred"}"#).unwrap();
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
fn scope_selector_rejects_invalid_structured_scope_path() {
    let err = ScopeSelector::parse(r#"{"kind":"path","path":"not/valid"}"#).unwrap_err();

    assert!(err.to_string().contains("Invalid scope"));
    assert!(err.to_string().contains("global"));
}

// ── Per-kind required-field validation (ALP-2476) ─────────────────

#[test]
fn scope_selector_rejects_path_kind_missing_path() {
    let err = ScopeSelector::parse(r#"{"kind":"path"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'path' requires field 'path'"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_subtree_missing_path() {
    let err = ScopeSelector::parse(r#"{"kind":"subtree"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'subtree' requires field 'path'"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_set_missing_paths() {
    let err = ScopeSelector::parse(r#"{"kind":"set"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'set' requires field 'paths'"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_set_empty_paths() {
    let err = ScopeSelector::parse(r#"{"kind":"set","paths":[]}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'set' requires a non-empty 'paths' array"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_project_missing_project() {
    let err = ScopeSelector::parse(r#"{"kind":"project"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'project' requires field 'project'"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_repo_missing_repo() {
    let err = ScopeSelector::parse(r#"{"kind":"repo","project":"helioy"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'repo' requires field 'repo'"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_session_missing_session() {
    let err = ScopeSelector::parse(r#"{"kind":"session","project":"helioy"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'session' requires field 'session'"),
        "got: {err}"
    );
}

// ── Per-kind extra-field rejection ────────────────────────────────

#[test]
fn scope_selector_rejects_all_kind_with_extra_fields() {
    let err = ScopeSelector::parse(r#"{"kind":"all","path":"global"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'all' does not accept field(s): path"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_subtree_with_paths_field() {
    let err =
        ScopeSelector::parse(r#"{"kind":"subtree","path":"global","paths":["global/project:x"]}"#)
            .unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'subtree' does not accept field(s): paths"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_set_with_path_field() {
    let err =
        ScopeSelector::parse(r#"{"kind":"set","paths":["global"],"path":"global"}"#).unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'set' does not accept field(s): path"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_cwd_inferred_with_path_field() {
    let err = ScopeSelector::parse(r#"{"kind":"cwd_inferred","cwd":"/tmp","path":"global"}"#)
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("scope kind 'cwd_inferred' does not accept field(s): path"),
        "got: {err}"
    );
}

#[test]
fn scope_selector_rejects_unknown_top_level_field() {
    let err = ScopeSelector::parse(r#"{"kind":"all","mystery":1}"#).unwrap_err();

    assert!(
        err.to_string().to_lowercase().contains("unknown"),
        "got: {err}"
    );
}

// ── Bare-keyword redirect (Codex-friendly errors) ─────────────────

#[test]
fn scope_selector_redirects_bare_all_keyword() {
    let err = ScopeSelector::parse("all").unwrap_err();

    assert!(
        err.to_string().contains(r#"use scope='{"kind":"all"}'"#),
        "got: {err}"
    );
}

#[test]
fn scope_selector_redirects_bare_subtree_keyword() {
    let err = ScopeSelector::parse("subtree").unwrap_err();

    assert!(err.to_string().contains(r#"use scope='{"kind":"subtree""#));
}

#[test]
fn scope_selector_redirects_bare_descendants_keyword() {
    let err = ScopeSelector::parse("descendants").unwrap_err();

    // Canonical name is subtree; the redirect points at it.
    assert!(err.to_string().contains(r#""kind":"subtree""#));
}

#[test]
fn scope_selector_redirects_bare_set_keyword() {
    let err = ScopeSelector::parse("set").unwrap_err();

    assert!(err.to_string().contains(r#"use scope='{"kind":"set""#));
}

#[test]
fn scope_selector_redirects_bare_project_keyword() {
    let err = ScopeSelector::parse("project").unwrap_err();

    assert!(err.to_string().contains(r#"use scope='{"kind":"project""#));
}

#[test]
fn scope_selector_redirects_bare_repo_keyword() {
    let err = ScopeSelector::parse("repo").unwrap_err();

    assert!(err.to_string().contains(r#"use scope='{"kind":"repo""#));
}

#[test]
fn scope_selector_redirects_bare_session_keyword() {
    let err = ScopeSelector::parse("session").unwrap_err();

    assert!(err.to_string().contains(r#"use scope='{"kind":"session""#));
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
    let subtree = ScopeSelector::Subtree(project_scope());
    let set = ScopeSelector::Set(vec![project_scope(), scope.clone()]);
    let all = ScopeSelector::All;

    assert_eq!(exact.requested_scope(), scope.as_str());
    assert_eq!(inferred.requested_scope(), "cwd_inferred");
    assert_eq!(subtree.requested_scope(), "global/project:helioy");
    assert_eq!(
        set.requested_scope(),
        "global/project:helioy,global/project:helioy/repo:context-matters"
    );
    assert_eq!(all.requested_scope(), "all");
}

#[test]
fn scope_selector_optional_scope_attaches_cwd_to_inferred_only() {
    let cwd = PathBuf::from("/tmp/helioy/context-matters");

    let omitted_without_cwd = ScopeSelector::from_optional_scope(None, None).unwrap();
    let inferred_without_cwd =
        ScopeSelector::from_optional_scope(Some(r#"{"kind":"cwd_inferred"}"#), None)
            .unwrap()
            .unwrap();
    let exact_without_cwd = ScopeSelector::from_optional_scope(
        Some(r#"{"kind":"path","path":"global/project:helioy/repo:context-matters"}"#),
        None,
    )
    .unwrap()
    .unwrap();
    let inferred =
        ScopeSelector::from_optional_scope(Some(r#"{"kind":"cwd_inferred"}"#), Some(cwd.clone()))
            .unwrap()
            .unwrap();
    let omitted = ScopeSelector::from_optional_scope(None, Some(cwd.clone()))
        .unwrap()
        .unwrap();
    let exact_err = ScopeSelector::from_optional_scope(
        Some(r#"{"kind":"path","path":"global/project:helioy/repo:context-matters"}"#),
        Some(cwd.clone()),
    )
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
