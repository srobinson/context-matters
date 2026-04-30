use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::scope::ScopePath;

/// Store-level scope predicate shared by read query paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ScopeFilter {
    Exact(ScopePath),
    AncestorWalk(ScopePath),
    Subtree(ScopePath),
    Set(Vec<ScopePath>),
    All,
}
