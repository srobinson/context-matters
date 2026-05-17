use std::{
    collections::{BTreeMap, HashSet},
    env,
    path::{Path, PathBuf},
    process::Command,
};

use cm_core::{CmError, ContextStore, Scope, ScopeFilter, ScopeKind, ScopePath};

use super::{
    BrowseScopeMode, CWD_INFERRED_SCOPE, ResolvedScopeSelection, ScopeResolution,
    ScopeResolutionCandidate, ScopeResolutionConfidence, ScopeSelector, segments::scope_segments,
};

const INFERRED_SCOPE_EXACT_MATCH_SCORE: i32 = 200;
const INFERRED_SCOPE_STRONG_SIGNAL_SCORE: i32 = 100;
const INFERRED_SCOPE_CWD_PROJECT_MATCH_SCORE: i32 = INFERRED_SCOPE_STRONG_SIGNAL_SCORE;
const INFERRED_SCOPE_PARENT_PROJECT_MATCH_SCORE: i32 = INFERRED_SCOPE_CWD_PROJECT_MATCH_SCORE + 1;
const INFERRED_SCOPE_WEAK_SIGNAL_SCORE: i32 = 30;
const INFERRED_SCOPE_FALLBACK_FLOOR_SCORE: i32 = 10;
const INFERRED_SCOPE_NO_SIGNAL_SCORE: i32 = 0;

const INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE: i32 = INFERRED_SCOPE_EXACT_MATCH_SCORE;
const INFERRED_SCOPE_MEDIUM_CONFIDENCE_MIN_SCORE: i32 = INFERRED_SCOPE_STRONG_SIGNAL_SCORE;
const INFERRED_SCOPE_LOW_CONFIDENCE_MIN_SCORE: i32 = INFERRED_SCOPE_NO_SIGNAL_SCORE + 1;

pub async fn resolve_scope_selection(
    store: &impl ContextStore,
    selector: &ScopeSelector,
) -> Result<ResolvedScopeSelection, CmError> {
    match selector {
        ScopeSelector::Path(scope_path) => Ok(ResolvedScopeSelection {
            scope_path: Some(scope_path.clone()),
            resolution: None,
            requested_scope: selector.requested_scope(),
        }),
        ScopeSelector::CwdInferred { cwd } => {
            let scopes = store.list_scopes(None).await?;
            let resolution = resolve_cwd_inferred_scope(&scopes, cwd.as_deref())?;
            Ok(ResolvedScopeSelection {
                scope_path: Some(resolution.resolved_scope.clone()),
                resolution: Some(resolution),
                requested_scope: selector.requested_scope(),
            })
        }
        ScopeSelector::Subtree(_) | ScopeSelector::Set(_) | ScopeSelector::All => {
            Err(CmError::Validation(format!(
                "scope kind '{}' selects multiple scopes and cannot be used as a single write target; use scope: \"{}\" or {{\"kind\":\"path\",\"path\":\"{}\"}} for one target",
                selector.kind_label(),
                selector.requested_scope(),
                selector.requested_scope()
            )))
        }
    }
}

pub async fn resolve_browse_scope(
    store: &impl ContextStore,
    selector: &ScopeSelector,
) -> Result<ResolvedScopeSelection, CmError> {
    resolve_scope_selection(store, selector).await
}

pub async fn resolve_scope_filter(
    store: &impl ContextStore,
    selector: &ScopeSelector,
) -> Result<ScopeFilter, CmError> {
    match selector {
        ScopeSelector::Path(_) | ScopeSelector::CwdInferred { .. } => {
            let resolved = resolve_scope_selection(store, selector).await?;
            let scope_path = resolved.read_scope_path()?.clone();
            Ok(ScopeFilter::Exact(scope_path))
        }
        ScopeSelector::Subtree(scope_path) => Ok(ScopeFilter::Subtree(scope_path.clone())),
        ScopeSelector::Set(scope_paths) => Ok(ScopeFilter::Set(scope_paths.clone())),
        ScopeSelector::All => Ok(ScopeFilter::All),
    }
}

fn resolve_cwd_inferred_scope(
    scopes: &[Scope],
    cwd: Option<&Path>,
) -> Result<ScopeResolution, CmError> {
    let env = SystemCwdEnvironment;
    resolve_cwd_inferred_scope_with_environment(scopes, cwd, &env)
}

fn resolve_cwd_inferred_scope_with_environment(
    scopes: &[Scope],
    cwd: Option<&Path>,
    env: &impl CwdEnvironment,
) -> Result<ScopeResolution, CmError> {
    let cwd = CwdParts::from_path(cwd, env)?;
    let candidates = filter_candidates(scopes, &cwd);

    let Some(top) = candidates.first() else {
        return Err(CmError::Validation(format!(
            "no candidate scope could be resolved for scope='{CWD_INFERRED_SCOPE}'"
        )));
    };

    let resolved_scope = top.scope.clone();
    let confidence = rate_confidence(confidence_score(&candidates));
    let signals = resolution_signals(&cwd, &candidates);

    Ok(ScopeResolution {
        requested_scope: CWD_INFERRED_SCOPE.to_owned(),
        resolved_scope,
        scope_mode: BrowseScopeMode::Resolved,
        confidence,
        candidates,
        signals,
    })
}

fn filter_candidates(scopes: &[Scope], cwd: &CwdParts) -> Vec<ScopeResolutionCandidate> {
    let mut candidate_paths = HashSet::new();
    let mut matching_repo_parents = HashSet::new();
    let scopes_by_path: BTreeMap<String, ScopePath> = scopes
        .iter()
        .map(|scope| (scope.path.as_str().to_owned(), scope.path.clone()))
        .collect();

    for scope in scopes {
        if scope.path.as_str() == "global" {
            candidate_paths.insert(scope.path.as_str().to_owned());
            continue;
        }

        if !cwd.has_cwd || scope.kind != ScopeKind::Repo {
            continue;
        }

        let segments = scope_segments(&scope.path);
        if segments.repo.as_deref() == cwd.basename.as_deref() {
            candidate_paths.insert(scope.path.as_str().to_owned());
            if let Some(parent) = parent_project_path(&scope.path) {
                matching_repo_parents.insert(parent.as_str().to_owned());
            }
        }
    }

    for scope in scopes {
        if !cwd.has_cwd || scope.kind != ScopeKind::Project {
            continue;
        }

        let segments = scope_segments(&scope.path);
        let project_matches_cwd = segments.project.as_ref().is_some_and(|project| {
            cwd.basename.as_ref() == Some(project) || cwd.parent_basename.as_ref() == Some(project)
        });
        let project_is_repo_parent = matching_repo_parents.contains(scope.path.as_str());

        if project_matches_cwd || project_is_repo_parent {
            candidate_paths.insert(scope.path.as_str().to_owned());
        }
    }

    let mut candidates: Vec<ScopeResolutionCandidate> = candidate_paths
        .into_iter()
        .filter_map(|path| scopes_by_path.get(&path).cloned())
        .map(|scope| score_candidate(scope, cwd))
        .filter(|candidate| {
            candidate.scope.leaf_kind() == ScopeKind::Global
                || candidate.score >= INFERRED_SCOPE_FALLBACK_FLOOR_SCORE
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.scope.depth().cmp(&a.scope.depth()))
            .then_with(|| a.scope.as_str().cmp(b.scope.as_str()))
    });

    candidates
}

#[derive(Debug, Default)]
struct CwdParts {
    has_cwd: bool,
    basename: Option<String>,
    parent_basename: Option<String>,
}

impl CwdParts {
    fn from_path(path: Option<&Path>, env: &impl CwdEnvironment) -> Result<Self, CmError> {
        let path = match path {
            Some(path) if path.as_os_str().is_empty() => {
                return Err(CmError::Validation("cwd cannot be empty".to_owned()));
            }
            Some(path) => path.to_path_buf(),
            None => env.current_dir()?,
        };

        if let Some(metadata) = env.git_metadata(&path) {
            return Ok(Self::from_normalized_path(metadata.scope_identity_root()));
        };

        Ok(Self::from_normalized_path(&path))
    }

    fn from_normalized_path(path: &Path) -> Self {
        let names: Vec<String> = path
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();

        Self {
            has_cwd: true,
            basename: names.last().cloned(),
            parent_basename: names.iter().rev().nth(1).cloned(),
        }
    }
}

trait CwdEnvironment {
    fn current_dir(&self) -> Result<PathBuf, CmError>;

    fn git_metadata(&self, cwd: &Path) -> Option<GitMetadata>;
}

struct SystemCwdEnvironment;

impl CwdEnvironment for SystemCwdEnvironment {
    fn current_dir(&self) -> Result<PathBuf, CmError> {
        env::current_dir().map_err(|e| {
            CmError::Validation(format!(
                "failed to determine current working directory: {e}"
            ))
        })
    }

    fn git_metadata(&self, cwd: &Path) -> Option<GitMetadata> {
        let worktree_root = git_path(cwd, &["rev-parse", "--show-toplevel"])?;
        let git_dir = git_path(cwd, &["rev-parse", "--git-dir"])?;
        let git_common_dir = git_path(cwd, &["rev-parse", "--git-common-dir"])?;

        Some(GitMetadata {
            worktree_root: absolutize(cwd, worktree_root),
            git_dir: absolutize(cwd, git_dir),
            git_common_dir: absolutize(cwd, git_common_dir),
        })
    }
}

#[derive(Debug, Clone)]
struct GitMetadata {
    worktree_root: PathBuf,
    git_dir: PathBuf,
    git_common_dir: PathBuf,
}

impl GitMetadata {
    fn scope_identity_root(&self) -> &Path {
        if self.is_linked_worktree()
            && self
                .git_common_dir
                .file_name()
                .is_some_and(|name| name == ".git")
            && let Some(source_root) = self.git_common_dir.parent()
        {
            return source_root;
        }

        &self.worktree_root
    }

    fn is_linked_worktree(&self) -> bool {
        self.git_dir != self.git_common_dir
    }
}

fn git_path(cwd: &Path, args: &[&str]) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }

    Some(PathBuf::from(path))
}

fn absolutize(base: &Path, path: PathBuf) -> PathBuf {
    let absolute = if path.is_absolute() {
        path
    } else {
        base.join(path)
    };
    std::fs::canonicalize(&absolute).unwrap_or(absolute)
}

fn parent_project_path(path: &ScopePath) -> Option<ScopePath> {
    path.ancestors().skip(1).find_map(|ancestor| {
        ScopePath::parse(ancestor)
            .ok()
            .filter(|path| path.leaf_kind() == ScopeKind::Project)
    })
}

fn score_candidate(scope: ScopePath, cwd: &CwdParts) -> ScopeResolutionCandidate {
    let mut score = 0;
    let mut matched = Vec::new();
    let segments = scope_segments(&scope);

    if cwd.has_cwd {
        if let Some(repo) = &cwd.basename
            && segments.repo.as_ref() == Some(repo)
        {
            score += INFERRED_SCOPE_EXACT_MATCH_SCORE;
            matched.push("repo".to_owned());
        }

        if let Some(project) = &cwd.parent_basename
            && segments.project.as_ref() == Some(project)
        {
            score += INFERRED_SCOPE_PARENT_PROJECT_MATCH_SCORE;
            matched.push("project_parent".to_owned());
        } else if let Some(project) = &cwd.basename
            && segments.project.as_ref() == Some(project)
        {
            score += INFERRED_SCOPE_CWD_PROJECT_MATCH_SCORE;
            matched.push("project_cwd".to_owned());
        }
    }

    match scope.leaf_kind() {
        ScopeKind::Repo => {
            score += INFERRED_SCOPE_WEAK_SIGNAL_SCORE;
            matched.push("specificity".to_owned());
        }
        ScopeKind::Project => {
            score += INFERRED_SCOPE_FALLBACK_FLOOR_SCORE;
            matched.push("project".to_owned());
        }
        ScopeKind::Global => {
            matched.push("fallback".to_owned());
        }
        ScopeKind::Session => {}
    }

    ScopeResolutionCandidate {
        scope,
        score,
        matched,
    }
}

fn confidence_score(candidates: &[ScopeResolutionCandidate]) -> i32 {
    let Some(top) = candidates.first() else {
        return INFERRED_SCOPE_NO_SIGNAL_SCORE;
    };

    if top.score >= INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE {
        let repo_ties = candidates
            .iter()
            .filter(|candidate| {
                candidate.score == top.score && candidate.scope.leaf_kind() == ScopeKind::Repo
            })
            .count();
        if repo_ties > 1 {
            return INFERRED_SCOPE_MEDIUM_CONFIDENCE_MIN_SCORE;
        }
    }

    top.score
}

fn rate_confidence(score: i32) -> ScopeResolutionConfidence {
    if score >= INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE {
        ScopeResolutionConfidence::High
    } else if score >= INFERRED_SCOPE_MEDIUM_CONFIDENCE_MIN_SCORE {
        ScopeResolutionConfidence::Medium
    } else if score >= INFERRED_SCOPE_LOW_CONFIDENCE_MIN_SCORE {
        ScopeResolutionConfidence::Low
    } else {
        ScopeResolutionConfidence::VeryLow
    }
}

fn resolution_signals(cwd: &CwdParts, candidates: &[ScopeResolutionCandidate]) -> Vec<String> {
    if !cwd.has_cwd {
        return vec!["no cwd supplied; using global fallback".to_owned()];
    }

    let mut signals = Vec::new();
    if let Some(repo) = &cwd.basename
        && candidates
            .iter()
            .any(|candidate| candidate.matched.iter().any(|matched| matched == "repo"))
    {
        signals.push(format!("cwd basename matched repo scope segment: {repo}"));
    }

    if let Some(project) = &cwd.parent_basename
        && candidates.iter().any(|candidate| {
            candidate
                .matched
                .iter()
                .any(|matched| matched == "project_parent")
        })
    {
        signals.push(format!(
            "cwd parent basename matched project scope segment: {project}"
        ));
    }

    if let Some(project) = &cwd.basename
        && candidates.iter().any(|candidate| {
            candidate
                .matched
                .iter()
                .any(|matched| matched == "project_cwd")
        })
    {
        signals.push(format!(
            "cwd basename matched project scope segment: {project}"
        ));
    }

    if let Some(top) = candidates.first() {
        let tied_top_count = candidates
            .iter()
            .filter(|candidate| candidate.score == top.score)
            .count();
        if tied_top_count > 1 {
            signals.push(format!(
                "ambiguous scope resolution; {tied_top_count} candidates share top score {}",
                top.score
            ));
        }
    }

    if candidates
        .first()
        .is_some_and(|candidate| candidate.scope.leaf_kind() == ScopeKind::Global)
    {
        signals.push("no local scope matched cwd; using global fallback".to_owned());
    }

    signals
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use cm_core::ScopePath;

    use super::*;

    #[derive(Debug)]
    struct FakeCwdEnvironment {
        current_dir: PathBuf,
        git_metadata: Option<GitMetadata>,
    }

    impl CwdEnvironment for FakeCwdEnvironment {
        fn current_dir(&self) -> Result<PathBuf, CmError> {
            Ok(self.current_dir.clone())
        }

        fn git_metadata(&self, _cwd: &Path) -> Option<GitMetadata> {
            self.git_metadata.clone()
        }
    }

    #[test]
    fn score_candidate_combines_repo_match_and_specificity() {
        let cwd = CwdParts::from_normalized_path(Path::new("/tmp/worktrees/context-matters"));
        let scope = ScopePath::parse("global/project:alpha/repo:context-matters").unwrap();

        let candidate = score_candidate(scope, &cwd);

        assert_eq!(
            candidate.score,
            INFERRED_SCOPE_EXACT_MATCH_SCORE + INFERRED_SCOPE_WEAK_SIGNAL_SCORE
        );
        assert_eq!(candidate.matched, vec!["repo", "specificity"]);
    }

    #[test]
    fn rate_confidence_maps_score_bands() {
        assert_eq!(
            rate_confidence(INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE),
            ScopeResolutionConfidence::High
        );
        assert_eq!(
            rate_confidence(INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE - 1),
            ScopeResolutionConfidence::Medium
        );
        assert_eq!(
            rate_confidence(INFERRED_SCOPE_MEDIUM_CONFIDENCE_MIN_SCORE),
            ScopeResolutionConfidence::Medium
        );
        assert_eq!(
            rate_confidence(INFERRED_SCOPE_MEDIUM_CONFIDENCE_MIN_SCORE - 1),
            ScopeResolutionConfidence::Low
        );
        assert_eq!(
            rate_confidence(INFERRED_SCOPE_LOW_CONFIDENCE_MIN_SCORE),
            ScopeResolutionConfidence::Low
        );
        assert_eq!(
            rate_confidence(INFERRED_SCOPE_NO_SIGNAL_SCORE),
            ScopeResolutionConfidence::VeryLow
        );
    }

    #[test]
    fn confidence_score_demotes_tied_repo_matches_to_medium_boundary() {
        let candidates = vec![
            ScopeResolutionCandidate {
                scope: ScopePath::parse("global/project:alpha/repo:context").unwrap(),
                score: INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE + INFERRED_SCOPE_WEAK_SIGNAL_SCORE,
                matched: vec![],
            },
            ScopeResolutionCandidate {
                scope: ScopePath::parse("global/project:beta/repo:context").unwrap(),
                score: INFERRED_SCOPE_HIGH_CONFIDENCE_MIN_SCORE + INFERRED_SCOPE_WEAK_SIGNAL_SCORE,
                matched: vec![],
            },
        ];

        assert_eq!(
            confidence_score(&candidates),
            INFERRED_SCOPE_MEDIUM_CONFIDENCE_MIN_SCORE
        );
    }

    #[test]
    fn cwd_parts_uses_environment_current_dir_when_cwd_is_missing() {
        let env = FakeCwdEnvironment {
            current_dir: PathBuf::from("/tmp/helioy/context-matters"),
            git_metadata: None,
        };

        let cwd = CwdParts::from_path(None, &env).unwrap();

        assert!(cwd.has_cwd);
        assert_eq!(cwd.basename.as_deref(), Some("context-matters"));
        assert_eq!(cwd.parent_basename.as_deref(), Some("helioy"));
    }

    #[test]
    fn cwd_parts_uses_linked_worktree_source_repo_identity() {
        let env = FakeCwdEnvironment {
            current_dir: PathBuf::from("/tmp/ignored"),
            git_metadata: Some(GitMetadata {
                worktree_root: PathBuf::from("/tmp/context-matters-worktrees/nancy-ALP-2054"),
                git_dir: PathBuf::from("/tmp/context-matters/.git/worktrees/nancy-ALP-2054"),
                git_common_dir: PathBuf::from("/tmp/context-matters/.git"),
            }),
        };

        let cwd = CwdParts::from_path(
            Some(Path::new("/tmp/context-matters-worktrees/nancy-ALP-2054")),
            &env,
        )
        .unwrap();

        assert_eq!(cwd.basename.as_deref(), Some("context-matters"));
        assert_eq!(cwd.parent_basename.as_deref(), Some("tmp"));
    }
}
