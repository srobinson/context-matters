//! Domain types and traits for the context-matters store.
//!
//! This crate defines the core abstractions with zero I/O dependencies.
//! The `ContextStore` trait uses synchronous method signatures.
//! Storage adapters (cm-store) wrap these in async where needed.

mod error;
pub mod query;
mod store;
mod types;

pub use error::{CmError, ScopePathError};
pub use query::{FtsQuery, QueryBuilder};
pub use store::ContextStore;
pub use types::{
    BrowseSort, Confidence, Entry, EntryFilter, EntryKind, EntryMeta, EntryRelation,
    MutationAction, MutationRecord, MutationSource, NewEntry, NewScope, PagedResult, Pagination,
    RelationKind, Scope, ScopeKind, ScopePath, StoreStats, TagCount, UpdateEntry, WriteContext,
};
