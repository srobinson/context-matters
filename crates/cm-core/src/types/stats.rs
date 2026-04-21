use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A tag with its usage count across active entries.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TagCount {
    pub tag: String,
    pub count: u64,
}

/// Aggregate statistics about the context store.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
    pub entries_by_kind: HashMap<String, u64>,

    /// Breakdown of active entries by scope path.
    pub entries_by_scope: HashMap<String, u64>,

    /// Breakdown of active entries by tag, pre-sorted by the store.
    pub entries_by_tag: Vec<TagCount>,

    /// Database file size in bytes (0 for in-memory databases).
    pub db_size_bytes: u64,
}
