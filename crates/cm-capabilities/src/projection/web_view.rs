//! Typed projection views for the cm-web HTTP API.
//!
//! Consumed by the cm-web Curator UI. Mirrors the information surfaced
//! by the YAML `format_browse_view` and `format_recall_view` formatters
//! as serialisable structs, so the web front-end renders the same
//! smart snippets, relative ages, and hoisted headers that the MCP
//! adapter shows. HTTP wiring, ts-rs regeneration, and frontend
//! consumption land in the follow-on issues ALP-1752 / 1753 / 1754.
//!
//! This module is the data-shape layer only. It does not touch the
//! store, the capability, or the HTTP surface. Each `project_web_*`
//! function is a pure transformation from the capability result
//! (and its originating request, where one is needed) to a view
//! struct.
//!
//! Every shared computation is delegated to the existing helpers in
//! the sibling projection modules. The YAML and web views cannot drift
//! on these because they read from the same source of truth. See the
//! DRY notes on [`super::browse_view::sort_as_str`],
//! [`super::recall_view::routing_explanation`], and
//! [`super::recall_view::search_tier_header_tag`] for the three helpers
//! that were promoted to `pub(crate)` specifically so this module could
//! reuse them verbatim.

use std::collections::BTreeMap;

mod browse;
mod get;
mod recall;
mod stats;
mod update;

pub use browse::*;
pub use get::*;
pub use recall::*;
pub use stats::*;
pub use update::*;

/// Convert the `usize`-valued histograms returned by
/// [`super::kind_histogram`], [`super::tag_histogram`], and
/// [`super::scope_histogram`] into `u32`-valued maps for the web view.
///
/// The YAML renderer only needs the `usize` form for its `render_histogram`
/// pass, but the web view must expose `u32` so ts-rs projects the field
/// as `Record<string, number>` rather than `Record<string, bigint>`.
/// Cast is lossless for any realistic result-set size; entries-per-slice
/// is bounded by the recall/browse limit, which tops out at `MAX_LIMIT`
/// (well under `u32::MAX`).
fn histogram_to_u32(src: BTreeMap<String, usize>) -> BTreeMap<String, u32> {
    src.into_iter().map(|(k, v)| (k, v as u32)).collect()
}
