use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ScopePathError;

/// The four levels of the scope hierarchy.
///
/// Ordering is significant: scopes must appear in hierarchical order
/// within a path (global < project < repo < session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Global = 0,
    Project = 1,
    Repo = 2,
    Session = 3,
}

impl ScopeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
            Self::Repo => "repo",
            Self::Session => "session",
        }
    }
}

impl std::fmt::Display for ScopeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ScopeKind {
    type Err = ScopePathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "global" => Ok(Self::Global),
            "project" => Ok(Self::Project),
            "repo" => Ok(Self::Repo),
            "session" => Ok(Self::Session),
            other => Err(ScopePathError::InvalidKind(other.to_string())),
        }
    }
}

/// Maximum byte length of a scope path.
const MAX_SCOPE_PATH_LEN: usize = 256;

/// Validate a scope identifier procedurally.
///
/// Matches: single alphanumeric char, or alphanumeric start/end with
/// hyphens allowed between. Pattern: `[a-z0-9]([a-z0-9-]*[a-z0-9])?`
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return false;
    }
    if !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// A validated, immutable scope path.
///
/// Invariants enforced at construction time:
/// - Starts with "global"
/// - Each segment after global follows `kind:identifier` format
/// - Kinds appear in ascending hierarchical order (global < project < repo < session)
/// - Each kind appears at most once
/// - Intermediate levels may be omitted (e.g., `global/project:x/session:y` is valid)
/// - Identifiers match `[a-z0-9]([a-z0-9-]*[a-z0-9])?`
/// - Total path length <= 256 bytes
///
/// # Examples
///
/// ```
/// use cm_core::ScopePath;
/// use cm_core::ScopeKind;
///
/// let path = ScopePath::parse("global/project:helioy/repo:nancyr").unwrap();
/// assert_eq!(path.leaf_kind(), ScopeKind::Repo);
/// assert_eq!(path.depth(), 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, as = "String")]
#[serde(try_from = "String", into = "String")]
pub struct ScopePath(String);

impl ScopePath {
    /// Parse and validate a scope path string.
    ///
    /// Returns `Err(ScopePathError)` if any invariant is violated.
    pub fn parse(input: &str) -> Result<Self, ScopePathError> {
        Self::validate(input)?;
        Ok(Self(input.to_string()))
    }

    /// Validate a scope path string without allocating.
    fn validate(input: &str) -> Result<(), ScopePathError> {
        if input.is_empty() {
            return Err(ScopePathError::Empty);
        }
        if input.len() > MAX_SCOPE_PATH_LEN {
            return Err(ScopePathError::TooLong {
                len: input.len(),
                max: MAX_SCOPE_PATH_LEN,
            });
        }

        let segments: Vec<&str> = input.split('/').collect();

        if segments[0] != "global" {
            return Err(ScopePathError::MissingGlobalRoot);
        }

        let mut prev_kind = ScopeKind::Global;

        for segment in &segments[1..] {
            let (kind_str, id) = segment
                .split_once(':')
                .ok_or_else(|| ScopePathError::MalformedSegment((*segment).to_string()))?;

            let kind: ScopeKind = kind_str.parse()?;

            if kind <= prev_kind {
                return Err(ScopePathError::OutOfOrder {
                    got: kind.as_str().to_string(),
                    after: prev_kind.as_str().to_string(),
                });
            }

            if !is_valid_identifier(id) {
                return Err(ScopePathError::InvalidIdentifier(id.to_string()));
            }

            prev_kind = kind;
        }

        Ok(())
    }

    /// The root scope. Always valid.
    pub fn global() -> Self {
        Self("global".to_string())
    }

    /// Return the raw path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return all ancestor paths from most specific to least specific.
    ///
    /// The path itself is included as the first element.
    /// The last element is always `"global"`.
    pub fn ancestors(&self) -> impl Iterator<Item = &str> {
        let s = self.0.as_str();
        let mut slash_positions = Vec::new();
        for (i, b) in s.bytes().enumerate() {
            if b == b'/' {
                slash_positions.push(i);
            }
        }

        let mut result: Vec<&str> = Vec::with_capacity(slash_positions.len() + 1);
        result.push(s);
        for &pos in slash_positions.iter().rev() {
            result.push(&s[..pos]);
        }
        result.into_iter()
    }

    /// Return the scope kind of the deepest (rightmost) segment.
    pub fn leaf_kind(&self) -> ScopeKind {
        if self.0 == "global" {
            return ScopeKind::Global;
        }
        let last_segment = self.0.rsplit('/').next().unwrap();
        let kind_str = last_segment.split(':').next().unwrap();
        // Safe: validated at construction
        kind_str.parse().unwrap()
    }

    /// Return the depth of the scope path.
    /// `"global"` has depth 1, `"global/project:x"` has depth 2, etc.
    pub fn depth(&self) -> usize {
        self.0.split('/').count()
    }
}

impl std::fmt::Display for ScopePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for ScopePath {
    type Err = ScopePathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for ScopePath {
    type Error = ScopePathError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate(&s)?;
        Ok(Self(s))
    }
}

impl From<ScopePath> for String {
    fn from(sp: ScopePath) -> Self {
        sp.0
    }
}

impl AsRef<str> for ScopePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A row from the `scopes` table.
///
/// Scopes define the hierarchy that entries belong to.
/// They must be created top-down: a scope's `parent_path`
/// must already exist before the scope can be created.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Scope {
    /// The full scope path, which is also the primary key.
    pub path: ScopePath,

    /// The kind of this scope's leaf segment.
    pub kind: ScopeKind,

    /// Human-readable label for this scope.
    pub label: String,

    /// Parent scope path. `None` for the root (`global`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_path: Option<ScopePath>,

    /// Optional JSONB metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,

    /// Timestamp of scope creation.
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new scope.
///
/// The `path` must be valid per `ScopePath` rules. If `parent_path` is
/// `Some`, the referenced scope must already exist.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NewScope {
    /// Full scope path (becomes the primary key).
    pub path: ScopePath,

    /// Human-readable label.
    pub label: String,

    /// Optional JSONB metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl NewScope {
    /// Derive `kind` from the leaf segment of the path.
    pub fn kind(&self) -> ScopeKind {
        self.path.leaf_kind()
    }

    /// Derive `parent_path` by removing the last segment.
    /// Returns `None` for the `global` root scope.
    pub fn parent_path(&self) -> Option<ScopePath> {
        let s = self.path.as_str();
        if s == "global" {
            return None;
        }
        let parent = &s[..s.rfind('/').unwrap()];
        // Safe: parent of a valid path is always valid
        Some(ScopePath::parse(parent).unwrap())
    }
}
