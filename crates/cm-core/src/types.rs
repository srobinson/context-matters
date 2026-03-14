use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{CmError, ScopePathError};

// ── ScopeKind ──────────────────────────────────────────────────────

/// The four levels of the scope hierarchy.
///
/// Ordering is significant: scopes must appear in hierarchical order
/// within a path (global < project < repo < session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

// ── ScopePath ──────────────────────────────────────────────────────

/// Maximum byte length of a scope path.
const MAX_SCOPE_PATH_LEN: usize = 256;

/// Validate a scope identifier procedurally (no regex dependency).
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ScopePath(String);

impl ScopePath {
    /// Parse and validate a scope path string.
    ///
    /// Returns `Err(ScopePathError)` if any invariant is violated.
    pub fn parse(input: &str) -> Result<Self, ScopePathError> {
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

        Ok(Self(input.to_string()))
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
        Self::parse(&s)
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

// ── Scope ──────────────────────────────────────────────────────────

/// A row from the `scopes` table.
///
/// Scopes define the hierarchy that entries belong to.
/// They must be created top-down: a scope's `parent_path`
/// must already exist before the scope can be created.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── NewScope ───────────────────────────────────────────────────────

/// Input for creating a new scope.
///
/// The `path` must be valid per `ScopePath` rules. If `parent_path` is
/// `Some`, the referenced scope must already exist.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ── EntryKind ──────────────────────────────────────────────────────

/// Classification of a context entry.
///
/// Each kind carries distinct semantic weight during recall.
/// `Feedback` entries receive highest priority
/// because they represent direct user corrections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Fact,
    Decision,
    Preference,
    Lesson,
    Reference,
    Feedback,
    Pattern,
    Observation,
}

impl EntryKind {
    /// Return the string representation used in SQL storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Preference => "preference",
            Self::Lesson => "lesson",
            Self::Reference => "reference",
            Self::Feedback => "feedback",
            Self::Pattern => "pattern",
            Self::Observation => "observation",
        }
    }
}

impl std::fmt::Display for EntryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EntryKind {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fact" => Ok(Self::Fact),
            "decision" => Ok(Self::Decision),
            "preference" => Ok(Self::Preference),
            "lesson" => Ok(Self::Lesson),
            "reference" => Ok(Self::Reference),
            "feedback" => Ok(Self::Feedback),
            "pattern" => Ok(Self::Pattern),
            "observation" => Ok(Self::Observation),
            other => Err(CmError::InvalidEntryKind(other.to_string())),
        }
    }
}

// ── Confidence ─────────────────────────────────────────────────────

/// Confidence level for a context entry.
///
/// Stored in the `meta` JSONB column. Affects recall priority:
/// `High` entries surface before `Low` entries at the same scope level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

// ── EntryMeta ──────────────────────────────────────────────────────

/// Structured metadata stored in the JSONB `meta` column.
///
/// The `extra` field captures any additional keys present in the JSON
/// that are not part of the known schema, providing forward-compatible
/// extensibility without schema changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntryMeta {
    /// Freeform tags for categorization and filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Confidence level. Affects recall priority ordering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,

    /// Attribution or provenance string (URL, file path, agent name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// ISO 8601 timestamp after which this entry is considered stale.
    /// The storage layer stores this value but does not enforce expiry.
    /// Expiry semantics are defined in the MCP tool layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,

    /// Numeric priority for manual ordering. Higher values surface first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,

    /// Forward-compatible extension fields.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// ── Entry ──────────────────────────────────────────────────────────

/// A complete context entry as stored in the database.
///
/// This type represents a row from the `entries` table with all columns populated.
/// Construct new entries via `NewEntry`; the store assigns `id`, `content_hash`,
/// `created_at`, `updated_at`, and `superseded_by`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// UUIDv7 identifier. Time-sortable, stored as lowercase hex TEXT.
    pub id: uuid::Uuid,

    /// Scope this entry belongs to. FK to `scopes.path`.
    pub scope_path: ScopePath,

    /// Classification of the entry.
    pub kind: EntryKind,

    /// Short summary for search results and progressive disclosure.
    pub title: String,

    /// Markdown content body.
    pub body: String,

    /// BLAKE3 hex digest of `scope_path + \0 + kind + \0 + body`.
    /// Used for deduplication. 64 lowercase hex characters.
    pub content_hash: String,

    /// Structured metadata (tags, confidence, source, expiry, priority).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<EntryMeta>,

    /// Attribution string in `source_type:identifier` format.
    pub created_by: String,

    /// Timestamp of entry creation.
    pub created_at: DateTime<Utc>,

    /// Timestamp of last modification. Maintained by database trigger.
    pub updated_at: DateTime<Utc>,

    /// If set, this entry has been superseded by the referenced entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<uuid::Uuid>,
}

// ── NewEntry ───────────────────────────────────────────────────────

/// Input for creating a new context entry.
///
/// The caller provides scope, kind, title, body, created_by, and optional metadata.
/// The store generates `id` (UUIDv7), `content_hash` (BLAKE3), and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEntry {
    /// Target scope path. Must reference an existing scope.
    pub scope_path: ScopePath,

    /// Classification.
    pub kind: EntryKind,

    /// Short summary.
    pub title: String,

    /// Markdown content body.
    pub body: String,

    /// Attribution string (`source_type:identifier`).
    pub created_by: String,

    /// Optional structured metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<EntryMeta>,
}

impl NewEntry {
    /// Compute the BLAKE3 content hash for deduplication.
    ///
    /// Hash input: `scope_path + \0 + kind + \0 + body`
    /// Returns lowercase hex string (64 chars).
    pub fn content_hash(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.scope_path.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(self.kind.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(self.body.as_bytes());
        hasher.finalize().to_hex().to_string()
    }
}

// ── UpdateEntry ────────────────────────────────────────────────────

/// Partial update to an existing entry.
///
/// Only fields set to `Some` are applied. `None` fields remain unchanged.
/// The `content_hash` is recomputed by the store if `body` or `kind` changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EntryKind>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<EntryMeta>,
}

// ── RelationKind ───────────────────────────────────────────────────

/// The kind of relationship between two entries.
///
/// Relations are directional: `source_id` has this relationship
/// to `target_id`. The composite primary key is
/// `(source_id, target_id, relation)`, allowing multiple
/// distinct relation types between the same pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// Source replaces target (target should have `superseded_by` set).
    Supersedes,
    /// Entries are topically related.
    RelatesTo,
    /// Source contradicts target. Signals a conflict for review.
    Contradicts,
    /// Source provides additional detail for target.
    Elaborates,
    /// Source depends on target being true or present.
    DependsOn,
}

impl RelationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supersedes => "supersedes",
            Self::RelatesTo => "relates_to",
            Self::Contradicts => "contradicts",
            Self::Elaborates => "elaborates",
            Self::DependsOn => "depends_on",
        }
    }
}

impl std::fmt::Display for RelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RelationKind {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "supersedes" => Ok(Self::Supersedes),
            "relates_to" => Ok(Self::RelatesTo),
            "contradicts" => Ok(Self::Contradicts),
            "elaborates" => Ok(Self::Elaborates),
            "depends_on" => Ok(Self::DependsOn),
            other => Err(CmError::InvalidRelationKind(other.to_string())),
        }
    }
}

// ── EntryRelation ──────────────────────────────────────────────────

/// A directional relationship between two entries.
///
/// Corresponds to a row in the `entry_relations` table.
/// Relations cascade-delete when either entry is removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryRelation {
    pub source_id: uuid::Uuid,
    pub target_id: uuid::Uuid,
    pub relation: RelationKind,
    pub created_at: DateTime<Utc>,
}

// ── Pagination ─────────────────────────────────────────────────────

/// Composite cursor for deterministic pagination.
///
/// Uses `(updated_at, id)` to avoid skipping entries when multiple
/// entries share the same `updated_at` timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationCursor {
    pub updated_at: DateTime<Utc>,
    pub id: uuid::Uuid,
}

/// Cursor-based pagination using `(updated_at, id)` ordering.
///
/// Results are ordered by `updated_at DESC, id DESC` (most recently
/// modified first, with UUID tiebreaker for deterministic ordering).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Maximum number of entries to return.
    pub limit: u32,

    /// Cursor for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<PaginationCursor>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            cursor: None,
        }
    }
}

// ── PagedResult ────────────────────────────────────────────────────

/// A paginated result set.
///
/// If `next_cursor` is `Some`, more results are available.
/// Pass it as `pagination.cursor` on the next request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResult<T> {
    /// The items on this page.
    pub items: Vec<T>,

    /// Total count of matching entries (across all pages).
    pub total: u64,

    /// Cursor for the next page, if more results exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<PaginationCursor>,
}

// ── EntryFilter ────────────────────────────────────────────────────

/// Query parameters for browsing and filtering entries.
///
/// All fields are optional. When multiple fields are set,
/// they combine with AND semantics. An empty filter returns
/// all active entries (where `superseded_by IS NULL`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntryFilter {
    /// Filter to a specific scope path (exact match, no ancestor walk).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_path: Option<ScopePath>,

    /// Filter by entry kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EntryKind>,

    /// Filter by tag (entry must have at least one matching tag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Filter by created_by attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,

    /// If true, include superseded (inactive) entries. Default: false.
    #[serde(default)]
    pub include_superseded: bool,

    /// Pagination parameters.
    #[serde(default)]
    pub pagination: Pagination,
}

// ── StoreStats ─────────────────────────────────────────────────────

/// Aggregate statistics about the context store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    /// Total number of active entries (superseded_by IS NULL).
    pub active_entries: u64,

    /// Total number of superseded entries.
    pub superseded_entries: u64,

    /// Number of scopes.
    pub scopes: u64,

    /// Number of relations.
    pub relations: u64,

    /// Breakdown of active entries by kind.
    pub entries_by_kind: std::collections::HashMap<String, u64>,

    /// Breakdown of active entries by scope path.
    pub entries_by_scope: std::collections::HashMap<String, u64>,

    /// Database file size in bytes (0 for in-memory databases).
    pub db_size_bytes: u64,
}
