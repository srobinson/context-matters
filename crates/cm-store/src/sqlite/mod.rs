//! SQLite implementation of the `ContextStore` trait.
//!
//! `CmStore` wraps dual sqlx connection pools (1 writer, 4 readers)
//! and implements the async `ContextStore` trait directly. All methods
//! use sqlx's async API, which dispatches queries to dedicated connection
//! threads via channels internally.
//!
//! ## Module layout
//!
//! - `parse`     - Row-to-type conversion (no I/O)
//! - `mutation`  - Mutation record helpers (snapshot, insert)
//! - `entry`     - Entry CRUD and lifecycle (create, get, update, supersede, forget)
//! - `query`     - Read-only queries (resolve_context, search, browse)
//! - `scope`     - Scope and relation operations
//! - `aggregate` - Stats, export, mutation history queries

mod aggregate;
mod cursor;
mod entry;
mod mutation;
pub(crate) mod parse;
mod query;
mod scope;

use chrono::{DateTime, Utc};
use cm_core::{
    CmError, ContextStore, Entry, EntryFilter, EntryKind, EntryRelation, MutationAction,
    MutationRecord, MutationSource, NewEntry, NewScope, PagedResult, RelationKind, Scope,
    ScopeKind, ScopePath, StoreStats, UpdateEntry, WriteContext,
};
use sqlx::sqlite::SqlitePool;
use uuid::Uuid;

/// SQLite-backed context store.
///
/// Holds dual connection pools: one for writes (max 1), one for reads (max 4).
/// Construct via `CmStore::new()` after creating pools with `schema::create_pools()`.
pub struct CmStore {
    pub(crate) write_pool: SqlitePool,
    pub(crate) read_pool: SqlitePool,
}

impl CmStore {
    /// Create a new store from pre-configured pools.
    pub fn new(write_pool: SqlitePool, read_pool: SqlitePool) -> Self {
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Access the write pool (for migrations, WAL checkpoint, etc.).
    pub fn write_pool(&self) -> &SqlitePool {
        &self.write_pool
    }

    /// Access the read pool.
    pub fn read_pool(&self) -> &SqlitePool {
        &self.read_pool
    }
}

impl ContextStore for CmStore {
    async fn create_entry(
        &self,
        new_entry: NewEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError> {
        self.do_create_entry(new_entry, ctx).await
    }

    async fn get_entry(&self, id: Uuid) -> Result<Entry, CmError> {
        self.do_get_entry(id).await
    }

    async fn get_entries(&self, ids: &[Uuid]) -> Result<Vec<Entry>, CmError> {
        self.do_get_entries(ids).await
    }

    async fn resolve_context(
        &self,
        scope_path: &ScopePath,
        kinds: &[EntryKind],
        limit: u32,
    ) -> Result<Vec<Entry>, CmError> {
        self.do_resolve_context(scope_path, kinds, limit).await
    }

    async fn search(
        &self,
        query: &str,
        scope_path: Option<&ScopePath>,
        limit: u32,
    ) -> Result<Vec<Entry>, CmError> {
        self.do_search(query, scope_path, limit).await
    }

    async fn browse(&self, filter: EntryFilter) -> Result<PagedResult<Entry>, CmError> {
        self.do_browse(filter).await
    }

    async fn update_entry(
        &self,
        id: Uuid,
        update: UpdateEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError> {
        self.do_update_entry(id, update, ctx).await
    }

    async fn supersede_entry(
        &self,
        old_id: Uuid,
        new_entry: NewEntry,
        ctx: &WriteContext,
    ) -> Result<Entry, CmError> {
        self.do_supersede_entry(old_id, new_entry, ctx).await
    }

    async fn forget_entry(&self, id: Uuid, ctx: &WriteContext) -> Result<(), CmError> {
        self.do_forget_entry(id, ctx).await
    }

    async fn create_relation(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        relation: RelationKind,
        ctx: &WriteContext,
    ) -> Result<EntryRelation, CmError> {
        self.do_create_relation(source_id, target_id, relation, ctx)
            .await
    }

    async fn get_relations_from(&self, source_id: Uuid) -> Result<Vec<EntryRelation>, CmError> {
        self.do_get_relations_from(source_id).await
    }

    async fn get_relations_to(&self, target_id: Uuid) -> Result<Vec<EntryRelation>, CmError> {
        self.do_get_relations_to(target_id).await
    }

    async fn create_scope(
        &self,
        new_scope: NewScope,
        ctx: &WriteContext,
    ) -> Result<Scope, CmError> {
        self.do_create_scope(new_scope, ctx).await
    }

    async fn get_scope(&self, path: &ScopePath) -> Result<Scope, CmError> {
        self.do_get_scope(path).await
    }

    async fn list_scopes(&self, kind: Option<ScopeKind>) -> Result<Vec<Scope>, CmError> {
        self.do_list_scopes(kind).await
    }

    async fn stats(&self) -> Result<StoreStats, CmError> {
        self.do_stats().await
    }

    async fn export(&self, scope_path: Option<&ScopePath>) -> Result<Vec<Entry>, CmError> {
        self.do_export(scope_path).await
    }

    async fn get_mutations(
        &self,
        entry_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MutationRecord>, CmError> {
        self.do_get_mutations(entry_id, limit, offset).await
    }

    async fn list_mutations(
        &self,
        entry_id: Option<Uuid>,
        action: Option<MutationAction>,
        source: Option<MutationSource>,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        limit: u32,
    ) -> Result<Vec<MutationRecord>, CmError> {
        self.do_list_mutations(entry_id, action, source, since, until, limit)
            .await
    }
}
