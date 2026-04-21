use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CmError;

/// The kind of relationship between two entries.
///
/// Relations are directional: `source_id` has this relationship
/// to `target_id`. The composite primary key is
/// `(source_id, target_id, relation)`, allowing multiple
/// distinct relation types between the same pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
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

/// A directional relationship between two entries.
///
/// Corresponds to a row in the `entry_relations` table.
/// Relations cascade-delete when either entry is removed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntryRelation {
    pub source_id: uuid::Uuid,
    pub target_id: uuid::Uuid,
    pub relation: RelationKind,
    pub created_at: DateTime<Utc>,
}
