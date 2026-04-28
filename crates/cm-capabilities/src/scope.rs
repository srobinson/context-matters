mod chain;
mod resolution;
mod types;

pub use chain::{ensure_scope_chain, ensure_scope_chain_with_status};
pub use resolution::resolve_browse_scope;
pub use types::{
    BrowseScopeInput, BrowseScopeMode, CWD_INFERRED_SCOPE, ResolvedBrowseScope,
    ResolvedScopeSelection, ScopeResolution, ScopeResolutionCandidate, ScopeResolutionConfidence,
    ScopeSelector,
};
