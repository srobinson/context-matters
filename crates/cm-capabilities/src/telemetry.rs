use std::time::Instant;

use cm_core::{CmError, ContentSearchPage, ContentSearchRequest, ScopeFilter, ScopePath};

use crate::{
    recall::{RecallRequest, RecallResult, RecallRouting},
    scope::ScopeSelector,
};

pub(crate) struct RetrievalLog {
    op: &'static str,
    scope_variant: &'static str,
    scope_paths: Vec<String>,
    query_len: usize,
    started_at: Instant,
}

impl RetrievalLog {
    pub(crate) fn from_recall_request(request: &RecallRequest) -> Self {
        let selector = request
            .scope
            .as_ref()
            .cloned()
            .unwrap_or_else(|| ScopeSelector::Path(ScopePath::global()));
        let (scope_variant, scope_paths) = scope_selector_fields(&selector);

        Self {
            op: "recall",
            scope_variant,
            scope_paths,
            query_len: request
                .query
                .as_deref()
                .map_or(0, |query| query.chars().count()),
            started_at: Instant::now(),
        }
    }

    pub(crate) fn from_search_request(request: &ContentSearchRequest) -> Self {
        let (scope_variant, scope_paths) = scope_filter_fields(&request.scope);

        Self {
            op: "search",
            scope_variant,
            scope_paths,
            query_len: request.query.chars().count(),
            started_at: Instant::now(),
        }
    }

    pub(crate) fn set_resolved_scope(&mut self, scope_path: Option<&ScopePath>) {
        if let Some(scope_path) = scope_path {
            self.scope_paths = vec![scope_path.as_str().to_owned()];
        }
    }

    pub(crate) fn emit_recall(&self, result: &Result<RecallResult, CmError>) {
        match result {
            Ok(result) => self.emit(
                result.entries.len(),
                recall_rank_source(result),
                error_variant(None),
            ),
            Err(err) => self.emit(0, "no_match", error_variant(Some(err))),
        }
    }

    pub(crate) fn emit_search(&self, result: &Result<ContentSearchPage, CmError>) {
        match result {
            Ok(page) => self.emit(
                page.items.len(),
                search_rank_source(page),
                error_variant(None),
            ),
            Err(err) => self.emit(0, "no_match", error_variant(Some(err))),
        }
    }

    fn emit(&self, result_count: usize, rank_source: &'static str, error_variant: &'static str) {
        let duration_ms = self.started_at.elapsed().as_millis() as u64;

        tracing::debug!(
            op = self.op,
            scope_variant = self.scope_variant,
            scope_paths = ?self.scope_paths,
            query_len = self.query_len,
            result_count = result_count,
            rank_source = rank_source,
            duration_ms = duration_ms,
            error_variant = error_variant,
            "retrieval operation completed"
        );
    }
}

fn scope_selector_fields(selector: &ScopeSelector) -> (&'static str, Vec<String>) {
    match selector {
        ScopeSelector::Path(path) => ("path", vec![path.as_str().to_owned()]),
        ScopeSelector::CwdInferred { .. } => ("cwd_inferred", Vec::new()),
        ScopeSelector::Subtree(path) => ("subtree", vec![path.as_str().to_owned()]),
        ScopeSelector::Set(paths) => ("set", paths_to_strings(paths)),
        ScopeSelector::All => ("all", Vec::new()),
    }
}

fn scope_filter_fields(filter: &ScopeFilter) -> (&'static str, Vec<String>) {
    match filter {
        ScopeFilter::Exact(path) => ("path", vec![path.as_str().to_owned()]),
        ScopeFilter::AncestorWalk(path) => ("path", vec![path.as_str().to_owned()]),
        ScopeFilter::Subtree(path) => ("subtree", vec![path.as_str().to_owned()]),
        ScopeFilter::Set(paths) => ("set", paths_to_strings(paths)),
        ScopeFilter::All => ("all", Vec::new()),
    }
}

fn paths_to_strings(paths: &[ScopePath]) -> Vec<String> {
    paths.iter().map(|path| path.as_str().to_owned()).collect()
}

fn recall_rank_source(result: &RecallResult) -> &'static str {
    if result.entries.is_empty() {
        return "no_match";
    }
    match result.routing {
        RecallRouting::Search => "fts",
        RecallRouting::TagScopeWalk
        | RecallRouting::ScopeResolve
        | RecallRouting::BrowseFallback => "recall_priority",
    }
}

fn search_rank_source(page: &ContentSearchPage) -> &'static str {
    if page.items.is_empty() {
        "no_match"
    } else {
        "fts"
    }
}

fn error_variant(error: Option<&CmError>) -> &'static str {
    match error {
        None => "none",
        Some(CmError::EntryNotFound(_)) => "entry_not_found",
        Some(CmError::ScopeNotFound(_)) => "scope_not_found",
        Some(CmError::DuplicateContent(_)) => "duplicate_content",
        Some(CmError::InvalidScopePath(_)) => "invalid_scope_path",
        Some(CmError::InvalidEntryKind(_)) => "invalid_entry_kind",
        Some(CmError::InvalidRelationKind(_)) => "invalid_relation_kind",
        Some(CmError::Validation(_)) => "validation",
        Some(CmError::InvalidOperationInput { .. }) => "invalid_operation_input",
        Some(CmError::ConstraintViolation(_)) => "constraint_violation",
        Some(CmError::Json(_)) => "json",
        Some(CmError::Database(_)) => "database",
        Some(CmError::Internal(_)) => "internal",
    }
}
