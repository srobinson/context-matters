use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{entry::EntryKind, scope::ScopePath};

/// Cursor-based pagination.
///
/// The `cursor` field is an opaque page token produced by the store.
/// Callers must not parse or construct cursors; pass the `next_cursor`
/// from a previous `PagedResult` to fetch the next page.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Pagination {
    /// Maximum number of entries to return.
    pub limit: u32,

    /// Opaque cursor from a previous `PagedResult::next_cursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            cursor: None,
        }
    }
}

/// A paginated result set.
///
/// If `next_cursor` is `Some`, more results are available.
/// Pass it as `pagination.cursor` on the next request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PagedResult<T: TS> {
    /// The items on this page.
    pub items: Vec<T>,

    /// Total count of matching entries (across all pages).
    pub total: u64,

    /// Opaque cursor for the next page, if more results exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Sort order for browse queries.
///
/// Each variant produces a deterministic total order with `id` as the
/// final tiebreaker. `Recent` (default) matches the legacy
/// `ORDER BY updated_at DESC, id DESC` behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum BrowseSort {
    /// Most recently updated first (`updated_at DESC, id DESC`).
    #[default]
    Recent,
    /// Least recently updated first (`updated_at ASC, id ASC`).
    Oldest,
    /// Title ascending, case-insensitive (`title ASC, id ASC`).
    TitleAsc,
    /// Title descending, case-insensitive (`title DESC, id DESC`).
    TitleDesc,
    /// Scope path ascending (`scope_path ASC, id ASC`).
    ScopeAsc,
    /// Scope path descending (`scope_path DESC, id DESC`).
    ScopeDesc,
    /// Kind ascending (`kind ASC, id ASC`).
    KindAsc,
    /// Kind descending (`kind DESC, id DESC`).
    KindDesc,
}

/// Query parameters for browsing and filtering entries.
///
/// All fields are optional. When multiple fields are set,
/// they combine with AND semantics. An empty filter returns
/// all active entries (where `superseded_by IS NULL`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
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

    /// Sort order. Default: `Recent` (most recently updated first).
    #[serde(default)]
    pub sort: BrowseSort,

    /// Pagination parameters.
    #[serde(default)]
    pub pagination: Pagination,
}
