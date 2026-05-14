use std::{path::PathBuf, str::FromStr};

use cm_core::{CmError, ScopePath};
use serde::{Deserialize, Serialize};

pub const CWD_INFERRED_SCOPE: &str = "cwd_inferred";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeSelector {
    Path(ScopePath),
    CwdInferred { cwd: Option<PathBuf> },
    Subtree(ScopePath),
    Set(Vec<ScopePath>),
    All,
}

impl ScopeSelector {
    pub fn parse(scope: &str) -> Result<Self, CmError> {
        let scope = scope.trim();
        if scope.is_empty() {
            return Err(CmError::Validation("scope cannot be empty".to_owned()));
        }
        if scope == "auto" {
            return Err(CmError::Validation(format!(
                "use scope='{{\"kind\":\"{CWD_INFERRED_SCOPE}\"}}' instead of scope='auto'"
            )));
        }
        if scope == CWD_INFERRED_SCOPE {
            return Ok(Self::CwdInferred { cwd: None });
        }
        if !scope.starts_with('{') {
            return Ok(Self::Path(ScopePath::parse(scope)?));
        }
        let value: serde_json::Value = serde_json::from_str(scope).map_err(|err| {
            CmError::Validation(format!(
                "scope must be structured JSON with a kind field: {err}"
            ))
        })?;
        if !value.is_object() || value.get("kind").is_none() {
            return Err(CmError::Validation(
                "scope must be structured JSON with a kind field".to_owned(),
            ));
        }
        serde_json::from_value(value)
            .map_err(|err| CmError::Validation(format!("Invalid scope: {err}")))
    }

    pub fn cwd_inferred(cwd: Option<PathBuf>) -> Self {
        Self::CwdInferred { cwd }
    }

    pub fn from_optional_scope(
        scope: Option<&str>,
        cwd: Option<PathBuf>,
    ) -> Result<Option<Self>, CmError> {
        match scope {
            Some(scope) => Ok(Some(Self::parse(scope)?.with_cwd(cwd)?)),
            None => Ok(validate_cwd(cwd)?.map(|cwd| Self::CwdInferred { cwd: Some(cwd) })),
        }
    }

    pub fn with_cwd(self, cwd: Option<PathBuf>) -> Result<Self, CmError> {
        match (self, cwd) {
            (Self::CwdInferred { cwd: existing }, cwd) => Ok(Self::CwdInferred {
                cwd: validate_cwd(cwd)?.or(existing),
            }),
            (selector, None) => Ok(selector),
            (selector, Some(_)) => Err(CmError::Validation(format!(
                "cwd can only be supplied with scope kind '{CWD_INFERRED_SCOPE}', not scope='{}'",
                selector.requested_scope()
            ))),
        }
    }

    pub fn requested_scope(&self) -> String {
        match self {
            Self::Path(scope_path) => scope_path.as_str().to_owned(),
            Self::CwdInferred { .. } => CWD_INFERRED_SCOPE.to_owned(),
            Self::Subtree(scope_path) => scope_path.as_str().to_owned(),
            Self::Set(scope_paths) => scope_paths
                .iter()
                .map(|scope_path| scope_path.as_str())
                .collect::<Vec<_>>()
                .join(","),
            Self::All => "all".to_owned(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Path(_) => "path",
            Self::CwdInferred { .. } => CWD_INFERRED_SCOPE,
            Self::Subtree(_) => "subtree",
            Self::Set(_) => "set",
            Self::All => "all",
        }
    }
}

fn validate_cwd(cwd: Option<PathBuf>) -> Result<Option<PathBuf>, CmError> {
    match cwd {
        Some(cwd) if cwd.as_os_str().is_empty() => {
            Err(CmError::Validation("cwd cannot be empty".to_owned()))
        }
        cwd => Ok(cwd),
    }
}

impl FromStr for ScopeSelector {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ScopeSelectorWire {
    Path {
        path: ScopePath,
    },
    CwdInferred {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,
    },
    Subtree {
        path: ScopePath,
    },
    Descendants {
        path: ScopePath,
    },
    Set {
        paths: Vec<ScopePath>,
    },
    Project {
        project: String,
    },
    Repo {
        project: String,
        repo: String,
    },
    Session {
        project: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repo: Option<String>,
        session: String,
    },
    All,
}

impl TryFrom<ScopeSelectorWire> for ScopeSelector {
    type Error = CmError;

    fn try_from(value: ScopeSelectorWire) -> Result<Self, Self::Error> {
        match value {
            ScopeSelectorWire::Path { path } => Ok(Self::Path(path)),
            ScopeSelectorWire::CwdInferred { cwd } => Ok(Self::CwdInferred { cwd }),
            ScopeSelectorWire::Subtree { path } | ScopeSelectorWire::Descendants { path } => {
                Ok(Self::Subtree(path))
            }
            ScopeSelectorWire::Set { paths } => Ok(Self::Set(paths)),
            ScopeSelectorWire::Project { project } => {
                Ok(Self::Path(scope_path_from_parts(&[("project", &project)])?))
            }
            ScopeSelectorWire::Repo { project, repo } => Ok(Self::Path(scope_path_from_parts(&[
                ("project", &project),
                ("repo", &repo),
            ])?)),
            ScopeSelectorWire::Session {
                project,
                repo,
                session,
            } => {
                let path = match repo {
                    Some(repo) => scope_path_from_parts(&[
                        ("project", &project),
                        ("repo", &repo),
                        ("session", &session),
                    ])?,
                    None => scope_path_from_parts(&[("project", &project), ("session", &session)])?,
                };
                Ok(Self::Path(path))
            }
            ScopeSelectorWire::All => Ok(Self::All),
        }
    }
}

fn scope_path_from_parts(parts: &[(&str, &str)]) -> Result<ScopePath, CmError> {
    let mut path = "global".to_owned();
    for (kind, id) in parts {
        path.push('/');
        path.push_str(kind);
        path.push(':');
        path.push_str(id);
    }
    Ok(ScopePath::parse(&path)?)
}

impl From<&ScopeSelector> for ScopeSelectorWire {
    fn from(value: &ScopeSelector) -> Self {
        match value {
            ScopeSelector::Path(path) => Self::Path { path: path.clone() },
            ScopeSelector::CwdInferred { cwd } => Self::CwdInferred { cwd: cwd.clone() },
            ScopeSelector::Subtree(path) => Self::Subtree { path: path.clone() },
            ScopeSelector::Set(paths) => Self::Set {
                paths: paths.clone(),
            },
            ScopeSelector::All => Self::All,
        }
    }
}

impl Serialize for ScopeSelector {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ScopeSelectorWire::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ScopeSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let serde_json::Value::String(scope) = value {
            return ScopeSelector::parse(&scope).map_err(serde::de::Error::custom);
        }
        let wire = ScopeSelectorWire::deserialize(value).map_err(serde::de::Error::custom)?;
        ScopeSelector::try_from(wire).map_err(serde::de::Error::custom)
    }
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
