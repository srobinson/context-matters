use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CmError, Entry, EntryFilter, EntryKind, EntryRelation, MutationAction, MutationRecord,
    MutationSource, NewEntry, NewScope, PagedResult, RelationKind, Scope, ScopeFilter, ScopeKind,
    ScopePath, StoreStats, UpdateEntry, WriteContext,
};

/// An `Entry` paired with a raw FTS5 relevance score.
///
/// Returned by [`ContextStore::do_search_ancestor_walk`]. The `score` field carries the raw
/// SQLite FTS5 `bm25(entries_fts)` / `rank` value as `f32`: a negative
/// float where **lower values indicate higher relevance** (better match).
/// The value is intentionally unnormalised at this layer so that downstream
/// callers can apply per-query or per-slice normalisation (e.g. min-max
/// scaling to `0..=1`) without losing ranking information.
#[derive(Debug, Clone)]
pub struct ScoredEntry {
    pub entry: Entry,
    pub score: f32,
}

/// Request for recall's FTS5 ancestor walk.
#[derive(Debug, Clone)]
pub struct AncestorWalkRequest {
    /// FTS5 query string. Supports prefix queries (`rust*`), phrase queries
    /// (`"scope path"`), and boolean operators (`AND`, `OR`, `NOT`).
    pub query: String,

    /// Singular scope. Results include entries at this exact scope and its ancestors.
    pub scope: ScopePath,

    /// Maximum number of results.
    pub limit: u32,
}

/// Request for content-first search across store-level scope predicates.
#[derive(Debug, Clone)]
pub struct ContentSearchRequest {
    /// FTS5 query string.
    pub query: String,

    /// Scope predicate. Unlike [`AncestorWalkRequest`], this can be wide
    /// (`Subtree`, `Set`, `All`).
    pub scope: ScopeFilter,

    /// Optional kind filter. If `None`, all kinds are included.
    pub kinds: Option<Vec<EntryKind>>,

    /// Optional tag filter. If `None`, all tags are included.
    pub tags: Option<Vec<String>>,

    /// Maximum number of results per page.
    pub limit: u32,

    /// Opaque cursor from a previous [`ContentSearchPage::next_cursor`].
    pub cursor: Option<String>,
}

/// A page of content-first search results.
#[derive(Debug, Clone)]
pub struct ContentSearchPage {
    /// Scored entries for this page, ordered best-match first.
    pub items: Vec<ScoredEntry>,

    /// Cursor for the next page, or `None` if no more results are available.
    pub next_cursor: Option<String>,
}

/// Strategy used when resolving `cwd_inferred` scope selectors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeInferenceStrategy {
    /// Infer from filesystem and git working directory signals.
    #[default]
    Filesystem,
    /// Disable cwd based inference and require explicit scope input.
    Custom,
    /// Reserved for a future Kubernetes aware resolver.
    K8s,
}

/// Recall ordering mode resolved at store startup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallRankingMode {
    /// Preserve existing scope-depth ordering.
    #[default]
    Legacy,
    /// Reserved for observe-only diffing.
    Shadow,
    /// Serve deterministic kind/confidence/priority ordering.
    Live,
}

impl RecallRankingMode {
    /// Parse a recall ranking mode from config or environment text.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "legacy" => Some(Self::Legacy),
            "shadow" => Some(Self::Shadow),
            "live" => Some(Self::Live),
            _ => None,
        }
    }

    /// Parse a recall ranking mode, returning legacy on invalid input.
    #[must_use]
    pub fn parse_or_legacy(raw: &str) -> Self {
        Self::parse(raw).unwrap_or_default()
    }
}

/// Per-entry position movement recorded by the recall shadow canary.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RecallShadowPositionDelta {
    pub id: Uuid,
    pub old_position: Option<u32>,
    pub new_position: Option<u32>,
    pub delta: i32,
}

/// Observe-only recall ranking canary row.
///
/// This type is pure data. Store adapters decide how to persist it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RecallShadowRecord {
    pub scope_path: Option<String>,
    pub query_hash: Option<String>,
    pub query_len: Option<u32>,
    pub routing: String,
    pub tier: Option<String>,
    pub k: u32,
    pub candidate_count: u32,
    pub top1_changed: bool,
    pub topk_overlap: f64,
    pub footrule: f64,
    pub mean_abs_position_delta: f64,
    pub position_deltas: Vec<RecallShadowPositionDelta>,
    pub old_ids: Vec<Uuid>,
    pub new_ids: Vec<Uuid>,
    pub window_truncated: bool,
    pub ranking_version: String,
    pub duration_ms: u32,
}

/// Async storage interface for context entries.
///
/// All methods are async and return `Result<T, CmError>`. Uses native
/// async fn in trait (stable since Rust 1.75). Consumers use generics
/// (`&impl ContextStore`) rather than `dyn ContextStore` because native
/// async traits do not support dynamic dispatch without boxing.
///
/// ## Bounds
///
/// `Send + Sync + 'static` enables:
/// - Wrapping in `Arc<S>` for shared ownership across async tasks
/// - Using `S` as a type parameter in structs stored in `Arc`
/// - Future compatibility with `tokio::spawn` if concurrency is added
///
/// ## Conventions
///
/// - Methods that query entries exclude superseded entries by default
///   unless the caller explicitly opts in via `EntryFilter::include_superseded`.
/// - Write methods (`create_entry`, `update_entry`, `supersede_entry`, `forget_entry`)
///   use the write pool (1 connection). Read methods use the read pool (4 connections).
/// - All IDs are UUIDv7, generated by the store on creation.
/// - All mutating methods accept `&WriteContext` for provenance tracking.
///   Entry methods (create, update, supersede, forget) write mutation records.
///   Scope and relation methods carry the parameter for consistency but do not
///   write mutation records.
#[allow(async_fn_in_trait)]
pub trait ContextStore: Send + Sync + 'static {
    /// Configured strategy for resolving `cwd_inferred` selectors.
    fn scope_inference_strategy(&self) -> ScopeInferenceStrategy {
        ScopeInferenceStrategy::Filesystem
    }

    /// Configured recall ordering mode.
    fn recall_ranking_mode(&self) -> RecallRankingMode {
        RecallRankingMode::Legacy
    }

    /// Persist an observe-only recall ranking canary row.
    ///
    /// Default no-op keeps pure and test stores source-compatible.
    async fn log_recall_shadow(&self, _record: RecallShadowRecord) -> Result<(), CmError> {
        Ok(())
    }

    // ── Entry CRUD ──────────────────────────────────────────────

    /// Create a new entry.
    ///
    /// Generates a UUIDv7 `id`, computes the BLAKE3 `content_hash`,
    /// and sets `created_at` and `updated_at` to the current timestamp.
    ///
    /// # Errors
    ///
    /// - `CmError::ScopeNotFound` if `new_entry.scope_path` does not exist in `scopes`.
    /// - `CmError::DuplicateContent` if an active entry with the same content hash exists.
    /// - `CmError::Validation` if title or body is empty.
    async fn create_entry(&self, new_entry: NewEntry, ctx: &WriteContext)
    -> Result<Entry, CmError>;

    /// Retrieve a single entry by ID.
    ///
    /// Returns the entry regardless of superseded status.
    ///
    /// # Errors
    ///
    /// - `CmError::EntryNotFound` if no entry with this ID exists.
    async fn get_entry(&self, id: Uuid) -> Result<Entry, CmError>;

    /// Retrieve multiple entries by IDs.
    ///
    /// Returns entries in the same order as the input IDs.
    /// Missing IDs are silently omitted from the result (no error).
    /// Returns entries regardless of superseded status.
    async fn get_entries(&self, ids: &[Uuid]) -> Result<Vec<Entry>, CmError>;

    /// Resolve context for a scope by walking up the hierarchy.
    ///
    /// Returns all active entries from the target scope and every
    /// ancestor scope, ordered by:
    /// 1. Scope depth (most specific first)
    /// 2. `updated_at` DESC within each scope level
    ///
    /// This is the primary recall method. MCP tools call this to gather
    /// all relevant context for a given working scope.
    ///
    /// # Arguments
    ///
    /// - `scope_path`: The target scope to resolve from.
    /// - `kinds`: Optional filter to specific entry kinds. If empty, all kinds are included.
    /// - `limit`: Maximum total entries to return across all scope levels.
    async fn resolve_context(
        &self,
        scope_path: &ScopePath,
        kinds: &[EntryKind],
        limit: u32,
    ) -> Result<Vec<Entry>, CmError>;

    /// Full-text search using FTS5 over a scope and its ancestors.
    ///
    /// Searches `title` and `body` fields using SQLite FTS5 `MATCH` syntax.
    /// Results are ranked by FTS5 relevance score and returned as
    /// [`ScoredEntry`], ordered best-match first (most negative `score`
    /// first).
    ///
    /// The request scope is always singular. Results include entries at the
    /// requested scope and its ancestors. Wider scope predicates belong on
    /// [`Self::do_content_search`].
    async fn do_search_ancestor_walk(
        &self,
        request: AncestorWalkRequest,
    ) -> Result<Vec<ScoredEntry>, CmError>;

    /// Content-first search across a store-level scope predicate.
    async fn do_content_search(
        &self,
        _request: ContentSearchRequest,
    ) -> Result<ContentSearchPage, CmError> {
        Err(CmError::InvalidOperationInput {
            op: "cx_search",
            reason: "content search store path is not implemented yet".to_owned(),
        })
    }

    /// Browse entries with filtering and pagination.
    ///
    /// Applies all filter criteria with AND semantics.
    /// Returns a paginated result set ordered by `updated_at DESC`.
    async fn browse(&self, filter: EntryFilter) -> Result<PagedResult<Entry>, CmError>;

    /// Partially update an existing entry.
    ///
    /// Only fields set to `Some` in `update` are modified.
    /// If `body` or `kind` changes, the `content_hash` is recomputed
    /// and checked for duplicates.
    /// `updated_at` is refreshed by the database trigger.
    ///
    /// # Errors
    ///
    /// - `CmError::EntryNotFound` if the entry does not exist.
    /// - `CmError::DuplicateContent` if the updated content hash matches another active entry.
    async fn update_entry(
        &self,
        id: Uuid,
        update: UpdateEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError>;

    /// Supersede an entry: soft-delete the old entry and create a replacement.
    ///
    /// Sets `superseded_by` on `old_id` to the new entry's ID.
    /// Creates a `Supersedes` relation from the new entry to the old entry.
    /// Executed as a single transaction.
    ///
    /// # Errors
    ///
    /// - `CmError::EntryNotFound` if `old_id` does not exist.
    /// - `CmError::DuplicateContent` if the new entry's content hash matches another active entry.
    async fn supersede_entry(
        &self,
        old_id: Uuid,
        new_entry: NewEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError>;

    /// Soft-delete an entry by marking it as forgotten.
    ///
    /// Sets `superseded_by` to the entry's own ID (self-referential),
    /// distinguishing a "forgotten" entry from one superseded by a replacement.
    ///
    /// # Errors
    ///
    /// - `CmError::EntryNotFound` if the entry does not exist.
    async fn forget_entry(&self, id: Uuid, ctx: &WriteContext) -> Result<(), CmError>;

    // ── Relations ───────────────────────────────────────────────

    /// Create a relation between two entries.
    ///
    /// # Errors
    ///
    /// - `CmError::EntryNotFound` if either `source_id` or `target_id` does not exist.
    /// - `CmError::ConstraintViolation` if the relation already exists.
    async fn create_relation(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        relation: RelationKind,
        ctx: &WriteContext,
    ) -> Result<EntryRelation, CmError>;

    /// Get all relations where the given entry is the source.
    async fn get_relations_from(&self, source_id: Uuid) -> Result<Vec<EntryRelation>, CmError>;

    /// Get all relations where the given entry is the target.
    async fn get_relations_to(&self, target_id: Uuid) -> Result<Vec<EntryRelation>, CmError>;

    /// Count outgoing relations for each id in `ids`, in a single batched query.
    ///
    /// Returns a map from entry id to the number of relations where that
    /// entry is the `source_id`. Ids with zero outgoing relations are
    /// **omitted** from the map (callers should treat absence as zero, e.g.
    /// `map.get(&id).copied().unwrap_or(0)`).
    ///
    /// Counts every `RelationKind` together (no per-kind breakdown). Only
    /// outgoing edges are counted because outgoing edges are what indicate
    /// an entry elaborates on or otherwise references other entries; revisit
    /// if incoming counts become useful for projection enrichment.
    ///
    /// The default implementation returns an empty map, so adapters that do
    /// not maintain a relations table can opt out without compile errors.
    /// `CmStore` overrides with a single batched `IN (?, ?, ...)` query and
    /// short-circuits to an empty map (no DB access) when `ids` is empty.
    async fn count_relations_for(&self, _ids: &[Uuid]) -> Result<HashMap<Uuid, u32>, CmError> {
        Ok(HashMap::new())
    }

    // ── Scopes ──────────────────────────────────────────────────

    /// Create a new scope.
    ///
    /// Derives `kind` and `parent_path` from the scope path.
    /// The parent scope must already exist (except for `global`).
    ///
    /// # Errors
    ///
    /// - `CmError::ScopeNotFound` if the parent scope does not exist.
    /// - `CmError::ConstraintViolation` if the scope already exists.
    async fn create_scope(&self, new_scope: NewScope, ctx: &WriteContext)
    -> Result<Scope, CmError>;

    /// Retrieve a scope by its path.
    ///
    /// # Errors
    ///
    /// - `CmError::ScopeNotFound` if no scope with this path exists.
    async fn get_scope(&self, path: &ScopePath) -> Result<Scope, CmError>;

    /// List all scopes, optionally filtered by kind.
    async fn list_scopes(&self, kind: Option<ScopeKind>) -> Result<Vec<Scope>, CmError>;

    // ── Aggregation ─────────────────────────────────────────────

    /// Return aggregate statistics about the store.
    async fn stats(&self) -> Result<StoreStats, CmError>;

    /// Export all active entries, optionally filtered by scope.
    ///
    /// Returns entries ordered by `scope_path ASC`, `created_at ASC`.
    /// Superseded entries are excluded.
    async fn export(&self, scope_path: Option<&ScopePath>) -> Result<Vec<Entry>, CmError>;

    // ── Mutations ───────────────────────────────────────────────

    /// Query mutation history for a specific entry, with pagination.
    async fn get_mutations(
        &self,
        entry_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MutationRecord>, CmError>;

    /// Query mutation history with filters, including timestamp range.
    async fn list_mutations(
        &self,
        entry_id: Option<Uuid>,
        action: Option<MutationAction>,
        source: Option<MutationSource>,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        limit: u32,
    ) -> Result<Vec<MutationRecord>, CmError>;
}
