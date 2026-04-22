use std::str::FromStr;

use cm_core::{CmError, ScopePath};

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
