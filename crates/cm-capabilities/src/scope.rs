use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    str::FromStr,
};

use cm_core::{CmError, ContextStore, NewScope, Scope, ScopeKind, ScopePath, WriteContext};

use crate::error::cm_err_to_string;

// ── Browse scope resolution ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseScopeInput {
    Auto,
    Exact(ScopePath),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BrowseScopeMode {
    #[default]
    Resolved,
}

impl BrowseScopeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
        }
    }
}

impl std::fmt::Display for BrowseScopeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BrowseScopeMode {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "resolved" => Ok(Self::Resolved),
            other => Err(CmError::Validation(format!(
                "invalid scope_mode: '{other}' (expected 'resolved')"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeResolutionConfidence {
    High,
    Medium,
    Low,
    VeryLow,
}

impl ScopeResolutionConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::VeryLow => "very_low",
        }
    }
}

impl std::fmt::Display for ScopeResolutionConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeResolution {
    pub requested_scope: String,
    pub resolved_scope: ScopePath,
    pub scope_mode: BrowseScopeMode,
    pub confidence: ScopeResolutionConfidence,
    pub candidates: Vec<ScopeResolutionCandidate>,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeResolutionCandidate {
    pub scope: ScopePath,
    pub score: i32,
    pub matched: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBrowseScope {
    pub scope_path: Option<ScopePath>,
    pub resolution: Option<ScopeResolution>,
}

pub async fn resolve_browse_scope(
    store: &impl ContextStore,
    request: &crate::browse::BrowseRequest,
) -> Result<ResolvedBrowseScope, CmError> {
    match normalize_browse_scope(request.scope.as_deref(), request.scope_path.as_ref())? {
        None => Ok(ResolvedBrowseScope {
            scope_path: None,
            resolution: None,
        }),
        Some(BrowseScopeInput::Exact(scope_path)) => Ok(ResolvedBrowseScope {
            scope_path: Some(scope_path),
            resolution: None,
        }),
        Some(BrowseScopeInput::Auto) => {
            let scopes = store.list_scopes(None).await?;
            let resolution =
                resolve_auto_scope(&scopes, request.cwd.as_deref(), request.scope_mode)?;
            Ok(ResolvedBrowseScope {
                scope_path: Some(resolution.resolved_scope.clone()),
                resolution: Some(resolution),
            })
        }
    }
}

fn normalize_browse_scope(
    scope: Option<&str>,
    scope_path: Option<&ScopePath>,
) -> Result<Option<BrowseScopeInput>, CmError> {
    let scope = scope.map(str::trim);
    if matches!(scope, Some("")) {
        return Err(CmError::Validation("scope cannot be empty".to_owned()));
    }

    match (scope, scope_path) {
        (None, None) => Ok(None),
        (None, Some(scope_path)) => Ok(Some(BrowseScopeInput::Exact(scope_path.clone()))),
        (Some("auto"), None) => Ok(Some(BrowseScopeInput::Auto)),
        (Some("auto"), Some(_)) => Err(CmError::Validation(
            "scope='auto' cannot be combined with scope_path".to_owned(),
        )),
        (Some(scope), None) => Ok(Some(BrowseScopeInput::Exact(ScopePath::parse(scope)?))),
        (Some(scope), Some(scope_path)) => {
            let explicit = ScopePath::parse(scope)?;
            if explicit == *scope_path {
                Ok(Some(BrowseScopeInput::Exact(scope_path.clone())))
            } else {
                Err(CmError::Validation(format!(
                    "scope conflicts with scope_path: scope='{explicit}', scope_path='{scope_path}'"
                )))
            }
        }
    }
}

fn resolve_auto_scope(
    scopes: &[Scope],
    cwd: Option<&Path>,
    scope_mode: BrowseScopeMode,
) -> Result<ScopeResolution, CmError> {
    let cwd = CwdParts::from_path(cwd);
    let scopes_by_path: BTreeMap<String, ScopePath> = scopes
        .iter()
        .map(|scope| (scope.path.as_str().to_owned(), scope.path.clone()))
        .collect();
    let mut candidate_paths = HashSet::new();
    let mut matching_repo_parents = HashSet::new();

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
        .map(|scope| score_candidate(scope, &cwd))
        .collect();

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.scope.depth().cmp(&a.scope.depth()))
            .then_with(|| a.scope.as_str().cmp(b.scope.as_str()))
    });

    let Some(top) = candidates.first() else {
        return Err(CmError::Validation(
            "no candidate scope could be resolved for scope='auto'".to_owned(),
        ));
    };

    let resolved_scope = top.scope.clone();
    let confidence = confidence_for(&candidates);
    let signals = resolution_signals(&cwd, &candidates);

    Ok(ScopeResolution {
        requested_scope: "auto".to_owned(),
        resolved_scope,
        scope_mode,
        confidence,
        candidates,
        signals,
    })
}

#[derive(Debug, Default)]
struct CwdParts {
    has_cwd: bool,
    basename: Option<String>,
    parent_basename: Option<String>,
}

impl CwdParts {
    fn from_path(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };

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

#[derive(Debug, Default)]
struct ScopeSegments {
    project: Option<String>,
    repo: Option<String>,
}

fn scope_segments(path: &ScopePath) -> ScopeSegments {
    let mut segments = ScopeSegments::default();
    for segment in path.as_str().split('/').skip(1) {
        let Some((kind, id)) = segment.split_once(':') else {
            continue;
        };
        match kind {
            "project" => segments.project = Some(id.to_owned()),
            "repo" => segments.repo = Some(id.to_owned()),
            _ => {}
        }
    }
    segments
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
        if segments.repo.as_deref() == cwd.basename.as_deref() {
            score += 200;
            matched.push("repo".to_owned());
        }

        if segments.project.as_deref() == cwd.basename.as_deref() {
            score += 100;
            matched.push("project_cwd".to_owned());
        } else if segments.project.as_deref() == cwd.parent_basename.as_deref() {
            score += 100;
            matched.push("project_parent".to_owned());
        }
    }

    match scope.leaf_kind() {
        ScopeKind::Repo => {
            score += 30;
            matched.push("specificity".to_owned());
        }
        ScopeKind::Project => {
            score += 10;
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

fn confidence_for(candidates: &[ScopeResolutionCandidate]) -> ScopeResolutionConfidence {
    let Some(top) = candidates.first() else {
        return ScopeResolutionConfidence::VeryLow;
    };

    if top.score >= 200 {
        let repo_ties = candidates
            .iter()
            .filter(|candidate| {
                candidate.score == top.score && candidate.scope.leaf_kind() == ScopeKind::Repo
            })
            .count();
        if repo_ties <= 1 {
            return ScopeResolutionConfidence::High;
        }
    }

    if top.score >= 100 {
        ScopeResolutionConfidence::Medium
    } else if top.score > 0 {
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

/// Ensure the full scope chain exists, creating missing scopes top-down.
///
/// When creating an entry with a scope path that does not exist, this
/// function creates the full scope chain automatically. This prevents
/// callers from needing to manage scope creation separately.
pub async fn ensure_scope_chain(
    store: &impl ContextStore,
    path: &ScopePath,
    ctx: &WriteContext,
) -> Result<(), String> {
    let ancestors: Vec<&str> = path.ancestors().collect();

    // Walk from root (last) to leaf (first)
    for ancestor_str in ancestors.into_iter().rev() {
        let ancestor = ScopePath::parse(ancestor_str).map_err(|e| cm_err_to_string(e.into()))?;
        match store.get_scope(&ancestor).await {
            Ok(_) => continue,
            Err(CmError::ScopeNotFound(_)) => {
                // Derive label from the last segment
                let label = ancestor_str
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.split(':').nth(1))
                    .unwrap_or(ancestor_str)
                    .to_owned();

                let new_scope = NewScope {
                    path: ancestor,
                    label,
                    meta: None,
                };
                store
                    .create_scope(new_scope, ctx)
                    .await
                    .map_err(cm_err_to_string)?;
            }
            Err(e) => return Err(cm_err_to_string(e)),
        }
    }
    Ok(())
}
