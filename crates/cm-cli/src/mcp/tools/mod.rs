//! Tool handlers for the 9 `cx_*` tools.
//!
//! Each handler receives a reference to the store and the raw JSON arguments,
//! validates inputs, calls the appropriate `ContextStore` trait methods, and
//! returns a pretty-printed JSON string or an error message with recovery guidance.

mod browse;
mod deposit;
mod export;
mod forget;
mod get;
mod recall;
mod stats;
mod store;
mod update;

pub use browse::cx_browse;
pub use deposit::cx_deposit;
pub use export::cx_export;
pub use forget::cx_forget;
pub use get::cx_get;
pub use recall::cx_recall;
pub use stats::cx_stats;
pub use store::cx_store;
pub use update::cx_update;

// Re-export shared helpers used by tool handlers.
pub(crate) use crate::shared::{
    default_created_by, default_scope, entry_to_browse_json, entry_to_full_json,
    entry_to_recall_json, parse_confidence,
};
