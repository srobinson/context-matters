//! `RecallResult` YAML-text formatter for MCP response bodies.
//!
//! Consumed by `cx_recall` (via the wire-swap sub that lands the YAML
//! envelope) to replace the double-encoded JSON-in-text response shape
//! with a compact, agent-legible YAML view. The target shape is described
//! in `research/cx-response-payload-redesign-context-matters.md` §5.2.2.
//!
//! The formatter is pure text: no I/O, no allocations beyond the output
//! string and its temporaries. The only non-deterministic input is the
//! reference `now` used for relative-age rendering, which is captured
//! once at the entry point and injected into [`format_recall_view_at`]
//! so snapshot tests can pin the `age:` column.
//!
//! ### BM25 score column
//!
//! Scores land on `RecallRow.score` only when `cm-store` takes the
//! `Search` routing branch, and the raw values are SQLite FTS5
//! `bm25()` output: negative, lower (more negative) means a better
//! match. This module min-max normalises them to `[0.0, 1.0]` with
//! an inversion, so the best match always renders as `1.00` regardless
//! of the raw range. See [`normalise_bm25`] for the formula.

use chrono::{DateTime, Utc};

mod entries;
mod header;
mod layout;
mod routing;
mod scoring;
mod trailers;

pub(crate) use routing::{routing_explanation, search_tier_header_tag};
pub use scoring::normalise_bm25;

use entries::render_entries;
use header::render_header;
use layout::Layout;
use trailers::render_trailers;

use crate::recall::{RecallRequest, RecallResult};

/// Render a [`RecallResult`] as YAML-annotated text for the `cx_recall`
/// MCP response body. See the module docstring for the target shape.
///
/// Captures `Utc::now()` once for relative-age formatting and delegates
/// to [`format_recall_view_at`]. Use the `_at` variant from tests that
/// need the rendered `age:` column to be deterministic.
pub fn format_recall_view(result: &RecallResult, request: &RecallRequest) -> String {
    format_recall_view_at(result, request, Utc::now())
}

/// Deterministic variant of [`format_recall_view`] that takes an explicit
/// reference `now` for relative-age rendering. Production callers should
/// prefer [`format_recall_view`]; this entry point exists so snapshot
/// tests can pin the `age:` column without touching the system clock.
pub fn format_recall_view_at(
    result: &RecallResult,
    request: &RecallRequest,
    now: DateTime<Utc>,
) -> String {
    let rows = result.entries.as_slice();
    let layout = Layout::new(rows, result, request, now);

    let mut out = String::with_capacity(1024);
    out.push_str("---\n");
    render_header(&mut out, result, request, &layout);
    out.push('\n');
    render_entries(&mut out, &layout);
    render_trailers(&mut out, result, &layout);
    out
}

#[cfg(test)]
#[path = "recall_view_tests.rs"]
mod tests;
