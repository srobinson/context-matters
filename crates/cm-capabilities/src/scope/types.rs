use std::{path::PathBuf, str::FromStr};

use cm_core::{CmError, ScopePath};

pub const CWD_INFERRED_SCOPE: &str = "cwd_inferred";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeSelector {
    Path(ScopePath),
    CwdInferred { cwd: Option<PathBuf> },
}

impl ScopeSelector {
    pub fn parse(scope: &str) -> Result<Self, CmError> {
        scope.parse()
    }

    pub fn cwd_inferred(cwd: Option<PathBuf>) -> Self {
        Self::CwdInferred { cwd }
    }

    pub fn requested_scope(&self) -> String {
        match self {
            Self::Path(scope_path) => scope_path.as_str().to_owned(),
            Self::CwdInferred { .. } => CWD_INFERRED_SCOPE.to_owned(),
        }
    }
}

impl FromStr for ScopeSelector {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let scope = s.trim();
        if scope.is_empty() {
            return Err(CmError::Validation("scope cannot be empty".to_owned()));
        }

        match scope {
            CWD_INFERRED_SCOPE => Ok(Self::CwdInferred { cwd: None }),
            "auto" => Err(CmError::Validation(format!(
                "scope='auto' has been removed; use scope='{CWD_INFERRED_SCOPE}'"
            ))),
            exact => Ok(Self::Path(ScopePath::parse(exact)?)),
        }
    }
}

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
pub struct ResolvedScopeSelection {
    pub scope_path: Option<ScopePath>,
    pub resolution: Option<ScopeResolution>,
    pub requested_scope: String,
}

impl ResolvedScopeSelection {
    pub fn read_scope_path(&self) -> Result<&ScopePath, CmError> {
        self.scope_path.as_ref().ok_or_else(|| {
            CmError::Validation(format!(
                "scope '{}' did not resolve to a scope path",
                self.requested_scope
            ))
        })
    }

    pub fn write_scope_path(&self) -> Result<&ScopePath, CmError> {
        let scope_path = self.read_scope_path()?;
        let Some(resolution) = &self.resolution else {
            return Ok(scope_path);
        };

        require_unique_high_confidence_resolution(resolution)?;
        Ok(scope_path)
    }
}

fn require_unique_high_confidence_resolution(resolution: &ScopeResolution) -> Result<(), CmError> {
    if resolution.confidence != ScopeResolutionConfidence::High {
        return Err(CmError::Validation(format!(
            "scope='{}' writes require high confidence inference",
            resolution.requested_scope
        )));
    }

    let top_score = resolution
        .candidates
        .iter()
        .map(|candidate| candidate.score)
        .max()
        .ok_or_else(|| {
            CmError::Validation(format!(
                "scope='{}' writes require one unique inference candidate",
                resolution.requested_scope
            ))
        })?;
    let top_count = resolution
        .candidates
        .iter()
        .filter(|candidate| candidate.score == top_score)
        .count();

    if top_count == 1 {
        Ok(())
    } else {
        Err(CmError::Validation(format!(
            "scope='{}' writes require one unique inference candidate",
            resolution.requested_scope
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBrowseScope {
    pub scope_path: Option<ScopePath>,
    pub resolution: Option<ScopeResolution>,
}
