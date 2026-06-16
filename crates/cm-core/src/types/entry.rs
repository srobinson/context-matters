use std::cmp::Reverse;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CmError;

use super::scope::ScopePath;

/// Classification of a context entry.
///
/// Each kind carries distinct semantic weight during recall.
/// `Feedback` entries receive highest priority
/// because they represent direct user corrections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
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

    /// Return the recall tier for deterministic priority ordering.
    ///
    /// Lower values rank first.
    pub fn rank_tier(&self) -> u8 {
        match self {
            Self::Feedback => 0,
            Self::Decision => 1,
            Self::Preference => 2,
            Self::Lesson => 3,
            Self::Pattern => 4,
            Self::Fact => 5,
            Self::Reference => 6,
            Self::Observation => 7,
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

/// Confidence level for a context entry.
///
/// Stored in the `meta` JSONB column. Affects recall priority:
/// `High` entries surface before `Low` entries at the same scope level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    /// Return the recall rank for deterministic priority ordering.
    ///
    /// Lower values rank first.
    pub fn recall_rank(&self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Low => 2,
        }
    }
}

/// Deterministic lexicographic recall key.
///
/// The key sorts ascending. `Reverse` wraps fields where larger values
/// should rank first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecallRankKey {
    kind_tier: u8,
    confidence_rank: u8,
    priority: Reverse<i32>,
    scope_depth: Reverse<usize>,
    updated_at: Reverse<DateTime<Utc>>,
    id: Reverse<uuid::Uuid>,
}

/// Structured metadata stored in the JSONB `meta` column.
///
/// The `extra` field captures any additional keys present in the JSON
/// that are not part of the known schema, providing forward-compatible
/// extensibility without schema changes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    #[ts(skip)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// A complete context entry as stored in the database.
///
/// This type represents a row from the `entries` table with all columns populated.
/// Construct new entries via `NewEntry`; the store assigns `id`, `content_hash`,
/// `created_at`, `updated_at`, and `superseded_by`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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

/// Build the deterministic recall ranking key for an entry.
///
/// Ordering is kind, confidence, priority, scope depth, recency, then id.
#[must_use]
pub fn recall_rank_key(entry: &Entry) -> RecallRankKey {
    let meta = entry.meta.as_ref();
    let confidence_rank = meta
        .and_then(|meta| meta.confidence)
        .unwrap_or(Confidence::Medium)
        .recall_rank();
    let priority = meta.and_then(|meta| meta.priority).unwrap_or(0);

    RecallRankKey {
        kind_tier: entry.kind.rank_tier(),
        confidence_rank,
        priority: Reverse(priority),
        scope_depth: Reverse(entry.scope_path.depth()),
        updated_at: Reverse(entry.updated_at),
        id: Reverse(entry.id),
    }
}

/// Input for creating a new context entry.
///
/// The caller provides scope, kind, title, body, created_by, and optional metadata.
/// The store generates `id` (UUIDv7), `content_hash` (BLAKE3), and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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

/// Partial update to an existing entry.
///
/// Only fields set to `Some` are applied. `None` fields remain unchanged.
/// The `content_hash` is recomputed by the store if `body` or `kind` changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
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
