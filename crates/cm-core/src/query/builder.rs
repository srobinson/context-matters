use crate::{EntryKind, ScopePath};

/// Builder for constructing structured entry queries.
///
/// Provides a fluent API for assembling filter criteria that
/// the storage layer translates into SQL WHERE clauses.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    scope_path: Option<ScopePath>,
    kinds: Vec<EntryKind>,
    tag: Option<String>,
    created_by: Option<String>,
    include_superseded: bool,
    limit: Option<u32>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter to entries at this exact scope path.
    pub fn scope(mut self, path: ScopePath) -> Self {
        self.scope_path = Some(path);
        self
    }

    /// Filter to entries of these kinds. Multiple kinds use OR logic.
    pub fn kinds(mut self, kinds: Vec<EntryKind>) -> Self {
        self.kinds = kinds;
        self
    }

    /// Filter to entries with this tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Filter to entries created by this attribution.
    pub fn created_by(mut self, created_by: impl Into<String>) -> Self {
        self.created_by = Some(created_by.into());
        self
    }

    /// Include superseded (inactive) entries in results.
    pub fn include_superseded(mut self, include: bool) -> Self {
        self.include_superseded = include;
        self
    }

    /// Set the maximum number of results.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn get_scope_path(&self) -> Option<&ScopePath> {
        self.scope_path.as_ref()
    }

    pub fn get_kinds(&self) -> &[EntryKind] {
        &self.kinds
    }

    pub fn get_tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn get_created_by(&self) -> Option<&str> {
        self.created_by.as_deref()
    }

    pub fn get_include_superseded(&self) -> bool {
        self.include_superseded
    }

    pub fn get_limit(&self) -> Option<u32> {
        self.limit
    }
}
