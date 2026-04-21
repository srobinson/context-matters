use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CmError;

/// Identifies where a write operation originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum MutationSource {
    /// MCP server tool handler (cx_store, cx_update, cx_forget, cx_deposit).
    Mcp,
    /// Direct CLI command.
    Cli,
    /// cm-web API handler.
    Web,
    /// Helix autonomous operations.
    Helix,
}

impl MutationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::Cli => "cli",
            Self::Web => "web",
            Self::Helix => "helix",
        }
    }
}

impl std::fmt::Display for MutationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for MutationSource {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mcp" => Ok(Self::Mcp),
            "cli" => Ok(Self::Cli),
            "web" => Ok(Self::Web),
            "helix" => Ok(Self::Helix),
            other => Err(CmError::Validation(format!(
                "invalid mutation source: '{other}'"
            ))),
        }
    }
}

/// Provenance context passed to every mutating ContextStore method.
///
/// Carries the originating source of a write operation. Extensible
/// for future fields (correlation_id, session_id, actor) without
/// breaking the trait signature.
#[derive(Debug, Clone, TS)]
#[ts(export)]
pub struct WriteContext {
    pub source: MutationSource,
}

impl WriteContext {
    pub fn new(source: MutationSource) -> Self {
        Self { source }
    }
}

/// Classifies the kind of mutation performed on an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum MutationAction {
    /// New entry created.
    Create,
    /// Existing entry fields updated.
    Update,
    /// Entry soft-deleted (superseded_by set to self).
    Forget,
    /// Entry superseded by a replacement entry.
    Supersede,
}

impl MutationAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Forget => "forget",
            Self::Supersede => "supersede",
        }
    }
}

impl std::fmt::Display for MutationAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for MutationAction {
    type Err = CmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "create" => Ok(Self::Create),
            "update" => Ok(Self::Update),
            "forget" => Ok(Self::Forget),
            "supersede" => Ok(Self::Supersede),
            other => Err(CmError::Validation(format!(
                "invalid mutation action: '{other}'"
            ))),
        }
    }
}

/// A recorded mutation on a context entry.
///
/// Written by cm-store in the same transaction as the write operation.
/// Snapshots are full JSON serializations of the `Entry` at that point in time.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MutationRecord {
    /// UUIDv7 identifier for this mutation record.
    pub id: uuid::Uuid,

    /// The entry that was mutated.
    pub entry_id: uuid::Uuid,

    /// What happened.
    pub action: MutationAction,

    /// Where the write originated.
    pub source: MutationSource,

    /// When the mutation occurred.
    pub timestamp: DateTime<Utc>,

    /// Full entry state before the mutation. `None` for `Create` (no prior state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_snapshot: Option<serde_json::Value>,

    /// Full entry state after the mutation. `None` only if the row was hard-deleted
    /// (which this system does not do). All four actions capture the post-state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_snapshot: Option<serde_json::Value>,
}
