//! Aggregation helpers: short ids, relative age, histograms, uniform-key
//! hoisting. Pure, no I/O.
//!
//! Used by the recall/browse YAML formatters to shape result-set headers
//! and row identifiers before rendering.

mod dedup;
mod drill_down;
mod formatting;
mod histogram;

pub use dedup::{CONTENT_HASH_DEDUP_PREFIX, compute_dedup_hints};
pub use drill_down::{DRILL_DOWN_THRESHOLD, DrillDownHint, compute_drill_down_hint};
pub use formatting::{fmt_with_commas, hex_prefix, relative_age};
pub use histogram::{
    count_desc_vec, count_desc_vec_u32, hoist_uniform, kind_histogram, render_histogram,
    render_pairs, scope_histogram, tag_histogram,
};

#[cfg(test)]
#[path = "aggregation_tests.rs"]
mod aggregation_tests;
