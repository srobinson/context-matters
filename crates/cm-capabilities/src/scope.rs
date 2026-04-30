mod chain;
mod resolution;
mod types;

pub use chain::{ensure_scope_chain, ensure_scope_chain_with_status};
pub use resolution::{resolve_browse_scope, resolve_scope_filter, resolve_scope_selection};
pub use types::{
    BrowseScopeMode, CWD_INFERRED_SCOPE, ResolvedScopeSelection, ScopeResolution,
    ScopeResolutionCandidate, ScopeResolutionConfidence, ScopeSelector,
};
