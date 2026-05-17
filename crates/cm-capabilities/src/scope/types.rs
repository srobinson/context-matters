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
        if let Some(hint) = bare_keyword_redirect(scope) {
            return Err(CmError::Validation(hint));
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

// ── Wire format ──────────────────────────────────────────────────
//
// The deserialization shape is a single flat object with an `enum`
// discriminator on `kind`. All payload fields are siblings and optional
// at the serde layer; per-kind required-field + extra-field validation
// runs in `ScopeSelectorWireIn::into_selector`.
//
// This shape is the only one advertised in the generated MCP tool
// schemas. OpenAI Codex's strict-mode tool validator rejects top-level
// `oneOf`/`anyOf`/`allOf`/`not` on any parameter (and `enum` without a
// companion `type`) before the model is ever invoked; Gemini's pipeline
// enforces the same restriction. See ALP-2476, openai/codex#2204.
//
// Serialization uses a separate compact enum that emits only the fields
// relevant to each variant. This keeps wire output identical to the
// pre-flat-shape contract so existing JSON consumers (web frontend,
// snapshot tests) round-trip unchanged.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeSelectorWireIn {
    kind: ScopeSelectorKind,
    #[serde(default)]
    path: Option<ScopePath>,
    #[serde(default)]
    paths: Option<Vec<ScopePath>>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    cwd: Option<PathBuf>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ScopeSelectorKind {
    Path,
    CwdInferred,
    #[serde(alias = "descendants")]
    Subtree,
    Set,
    Project,
    Repo,
    Session,
    All,
}

impl ScopeSelectorKind {
    fn label(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::CwdInferred => CWD_INFERRED_SCOPE,
            Self::Subtree => "subtree",
            Self::Set => "set",
            Self::Project => "project",
            Self::Repo => "repo",
            Self::Session => "session",
            Self::All => "all",
        }
    }
}

impl ScopeSelectorWireIn {
    fn into_selector(self) -> Result<ScopeSelector, CmError> {
        let kind = self.kind;
        match kind {
            ScopeSelectorKind::All => {
                self.reject_unexpected(&[], kind)?;
                Ok(ScopeSelector::All)
            }
            ScopeSelectorKind::CwdInferred => {
                self.reject_unexpected(&["cwd"], kind)?;
                Ok(ScopeSelector::CwdInferred { cwd: self.cwd })
            }
            ScopeSelectorKind::Path => {
                let path = self.required_path(kind)?;
                self.reject_unexpected(&["path"], kind)?;
                Ok(ScopeSelector::Path(path))
            }
            ScopeSelectorKind::Subtree => {
                let path = self.required_path(kind)?;
                self.reject_unexpected(&["path"], kind)?;
                Ok(ScopeSelector::Subtree(path))
            }
            ScopeSelectorKind::Set => {
                let paths = self.paths.as_ref().ok_or_else(|| missing("paths", kind))?;
                if paths.is_empty() {
                    return Err(CmError::Validation(format!(
                        "scope kind '{}' requires a non-empty 'paths' array",
                        kind.label()
                    )));
                }
                self.reject_unexpected(&["paths"], kind)?;
                Ok(ScopeSelector::Set(self.paths.unwrap()))
            }
            ScopeSelectorKind::Project => {
                let project = self.required_string(&self.project, "project", kind)?;
                self.reject_unexpected(&["project"], kind)?;
                Ok(ScopeSelector::Path(scope_path_from_parts(&[(
                    "project", project,
                )])?))
            }
            ScopeSelectorKind::Repo => {
                let project = self.required_string(&self.project, "project", kind)?;
                let repo = self.required_string(&self.repo, "repo", kind)?;
                self.reject_unexpected(&["project", "repo"], kind)?;
                Ok(ScopeSelector::Path(scope_path_from_parts(&[
                    ("project", project),
                    ("repo", repo),
                ])?))
            }
            ScopeSelectorKind::Session => {
                let project = self.required_string(&self.project, "project", kind)?;
                let session = self.required_string(&self.session, "session", kind)?;
                self.reject_unexpected(&["project", "repo", "session"], kind)?;
                let path = match &self.repo {
                    Some(repo) => scope_path_from_parts(&[
                        ("project", project),
                        ("repo", repo),
                        ("session", session),
                    ])?,
                    None => scope_path_from_parts(&[("project", project), ("session", session)])?,
                };
                Ok(ScopeSelector::Path(path))
            }
        }
    }

    fn required_path(&self, kind: ScopeSelectorKind) -> Result<ScopePath, CmError> {
        self.path.clone().ok_or_else(|| missing("path", kind))
    }

    fn required_string<'a>(
        &self,
        value: &'a Option<String>,
        field: &'static str,
        kind: ScopeSelectorKind,
    ) -> Result<&'a str, CmError> {
        value.as_deref().ok_or_else(|| missing(field, kind))
    }

    fn reject_unexpected(&self, allowed: &[&str], kind: ScopeSelectorKind) -> Result<(), CmError> {
        let candidates: [(&str, bool); 6] = [
            ("path", self.path.is_some()),
            ("paths", self.paths.is_some()),
            ("project", self.project.is_some()),
            ("repo", self.repo.is_some()),
            ("session", self.session.is_some()),
            ("cwd", self.cwd.is_some()),
        ];
        let unexpected: Vec<&str> = candidates
            .into_iter()
            .filter_map(|(name, present)| (present && !allowed.contains(&name)).then_some(name))
            .collect();
        if unexpected.is_empty() {
            return Ok(());
        }
        Err(CmError::Validation(format!(
            "scope kind '{}' does not accept field(s): {}",
            kind.label(),
            unexpected.join(", ")
        )))
    }
}

fn missing(field: &str, kind: ScopeSelectorKind) -> CmError {
    CmError::Validation(format!(
        "scope kind '{}' requires field '{field}'",
        kind.label()
    ))
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

// Compact output shape: only the fields relevant to each variant are
// emitted. Preserves the historical wire format consumed by the web
// frontend and snapshot tests.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScopeSelectorWireOut {
    Path {
        path: ScopePath,
    },
    CwdInferred {
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,
    },
    Subtree {
        path: ScopePath,
    },
    Set {
        paths: Vec<ScopePath>,
    },
    All,
}

impl From<&ScopeSelector> for ScopeSelectorWireOut {
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
        ScopeSelectorWireOut::from(self).serialize(serializer)
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
        let wire = ScopeSelectorWireIn::deserialize(value).map_err(serde::de::Error::custom)?;
        wire.into_selector().map_err(serde::de::Error::custom)
    }
}

/// Friendly redirect for legacy bare-keyword scope inputs.
///
/// These were never officially advertised, but Codex and other agents
/// occasionally probed them when the prior schema's prose listed kinds
/// inline (e.g. `scope: "all"`). They now produce an actionable error
/// pointing at the canonical object shape rather than the generic
/// `"scope path must start with 'global'"` parser message.
fn bare_keyword_redirect(scope: &str) -> Option<String> {
    let suggestion = match scope {
        "all" => r#"{"kind":"all"}"#.to_owned(),
        "subtree" => r#"{"kind":"subtree","path":"<scope path>"}"#.to_owned(),
        "descendants" => r#"{"kind":"subtree","path":"<scope path>"}"#.to_owned(),
        "set" => r#"{"kind":"set","paths":["<path1>","<path2>"]}"#.to_owned(),
        "project" => r#"{"kind":"project","project":"<name>"}"#.to_owned(),
        "repo" => r#"{"kind":"repo","project":"<name>","repo":"<name>"}"#.to_owned(),
        "session" => r#"{"kind":"session","project":"<name>","session":"<name>"}"#.to_owned(),
        _ => return None,
    };
    Some(format!(
        "use scope='{suggestion}' instead of scope='{scope}'"
    ))
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
