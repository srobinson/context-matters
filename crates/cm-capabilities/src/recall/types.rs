use std::collections::HashMap;

use cm_core::EntryKind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::projection::RecallRow;
use crate::scope::ScopeSelector;

pub const DEFAULT_RECALL_SCOPE: &str = "global";
pub const RECALL_SCOPE_DEFAULT_ADVISORY: &str =
    "no --scope specified, searching 'global'. run `cm stats` to list all scopes.";

/// Input for a recall operation.
#[derive(Debug, Clone, Default)]
pub struct RecallRequest {
    pub query: Option<String>,
    /// Omitted scope defaults to [`DEFAULT_RECALL_SCOPE`] inside the capability.
    pub scope: Option<ScopeSelector>,
    pub kinds: Vec<EntryKind>,
    pub tags: Vec<String>,
    pub limit: u32,
    pub max_tokens: Option<u32>,
}

/// Which code path was taken during recall routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallRouting {
    Search,
    TagScopeWalk,
    ScopeResolve,
    BrowseFallback,
}

/// Which tier of the FTS5 fallback cascade produced the returned rows.
///
/// The recall cascade tries progressively broader query shapes until one
/// returns a non-empty row set. `Exact` is the strictest (implicit AND on
/// raw tokens), `Prefix` relaxes each token to a prefix match, and
/// `SplitOr` joins tokens with `OR` so any shared term will hit. `None` is
/// reserved for the case where all three tiers were tried and none
/// returned rows (distinct from `RecallResult.tier == None`, which signals
/// the cascade was never entered because the routing was not `Search`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum SearchTier {
    Exact,
    Prefix,
    SplitOr,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecallAdvisory {
    ScopeDefaulted { applied: String },
}

impl RecallAdvisory {
    pub fn body(&self) -> &'static str {
        match self {
            Self::ScopeDefaulted { .. } => RECALL_SCOPE_DEFAULT_ADVISORY,
        }
    }
}

/// Result of a recall operation.
#[derive(Debug, Clone)]
pub struct RecallResult {
    /// Recall rows, each pairing an `Entry` with an optional FTS5 score.
    /// Scores are populated only on the `Search` routing branch; every
    /// other branch leaves `score` as `None`.
    pub entries: Vec<RecallRow>,
    pub scope_chain: Vec<String>,
    /// Per-scope hit counts for the requested or defaulted scope, ordered from
    /// most specific ancestor to broadest.
    pub scope_hits: Vec<(String, usize)>,
    /// Total estimated tokens across all returned entries, using full bodies.
    pub token_estimate: u32,
    pub routing: RecallRouting,
    /// Which tier of the FTS5 cascade produced the rows. `Some(_)` only when
    /// `routing == Search`; non-search routings leave this as `None`.
    pub tier: Option<SearchTier>,
    pub candidates_before_filter: usize,
    pub fetch_limit_used: u32,
    /// Outgoing relation counts per row id, populated by a single
    /// `ContextStore::count_relations_for` batch call after final selection.
    /// Ids with zero outgoing edges are omitted from the map.
    pub relation_counts: HashMap<Uuid, u32>,
    pub advisories: Vec<RecallAdvisory>,
}
