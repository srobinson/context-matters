use std::fmt::Write as _;

use super::super::{DrillDownHint, compute_drill_down_hint, estimate_tokens};
use super::layout::Layout;
use super::routing::{routing_advice, search_tier_trailer};
use crate::recall::{RecallResult, RecallRouting};

/// Per-row body size above which the formatter emits a `cx_get(...)` hint
/// suggesting the caller fetch full content separately. Tuned to slightly
/// below the recall-default per-row snippet budget.
const TOKEN_HINT_THRESHOLD: u32 = 1024;

/// Maximum number of short ids the `cx_get(...)` hint lists explicitly
/// before appending `...`. Keeps the trailer bounded on large result sets.
const TOKEN_HINT_MAX_IDS: usize = 6;

pub(super) fn render_trailers(out: &mut String, result: &RecallResult, layout: &Layout) {
    out.push('\n');

    let (routing_str, routing_advice) = routing_advice(&result.routing);
    let _ = writeln!(out, "# routing: {routing_str} - {routing_advice}");

    // Tier advisory teaches the caller that the result set was
    // produced by a fallback query shape (prefix or split_or) rather
    // than the exact implicit-AND that they wrote. Fires only on the
    // two fallback tiers; exact is the happy path, none already
    // surfaces through the `no matches` trailer below.
    if matches!(result.routing, RecallRouting::Search)
        && let Some(tier) = result.tier
        && let Some(line) = search_tier_trailer(tier)
    {
        let _ = writeln!(out, "{line}");
    }

    // Faceted drill-down advisory: when one kind or tag accounts for
    // 60% or more of the result set, append a one-line `narrow:` hint
    // that shows the caller the exact `cx_recall(...)` shape to re-issue
    // pre-narrowed to that facet. The advisory is suppressed for
    // empty / single-row result sets via the `total < 2` guard inside
    // `compute_drill_down_hint`, so this call is safe to make
    // unconditionally before the empty-result early return below.
    if let Some(hint) =
        compute_drill_down_hint(&layout.kind_hist, &layout.tag_hist, layout.rows.len())
    {
        let _ = writeln!(out, "{}", format_recall_drill_down(&hint, layout.query));
    }

    if layout.rows.is_empty() {
        let _ = writeln!(
            out,
            "# no matches - widen the scope, drop filters, or try OR between synonyms"
        );
        return;
    }

    // Two-phase hint: list ids of rows whose body alone exceeds
    // `TOKEN_HINT_THRESHOLD`, encouraging the caller to fetch full
    // content via `cx_get(id=...)` rather than re-render the snippet.
    let big_ids: Vec<String> = layout
        .rows
        .iter()
        .filter(|row| estimate_tokens(&row.entry.body) > TOKEN_HINT_THRESHOLD)
        .map(|row| row.entry.id.to_string())
        .collect();
    if big_ids.is_empty() {
        return;
    }
    let (shown, truncated) = if big_ids.len() > TOKEN_HINT_MAX_IDS {
        (&big_ids[..TOKEN_HINT_MAX_IDS], true)
    } else {
        (big_ids.as_slice(), false)
    };
    let ids_rendered = shown
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let tail = if truncated { ", ..." } else { "" };
    let _ = writeln!(out, "# cx_get(id={ids_rendered}{tail}) for full bodies");
}

/// Render the recall-side drill-down advisory line for a populated
/// [`DrillDownHint`]. The output shape mirrors the cx_recall MCP
/// surface so the caller can copy the suggested call verbatim:
///
/// ```text
/// # narrow: cx_recall(query="snippet strategy", kinds=["decision"]) - 2 of 3 results are decision
/// # narrow: cx_recall(query="snippet strategy", tags=["session-log"]) - 14 of 20 results are tagged session-log
/// # narrow: cx_recall(kinds=["fact"]) - 2 of 2 results are fact
/// ```
///
/// `query` is `None` when the recall request had no free-text query
/// (tag/scope-only routing), in which case the rendered call drops the
/// `query=...` argument entirely. The trailing prose qualifier toggles
/// `tagged ` for tag dominance so the line reads naturally for both
/// facets ("results are decision" / "results are tagged session-log").
fn format_recall_drill_down(hint: &DrillDownHint, query: Option<&str>) -> String {
    let DrillDownHint {
        facet,
        value,
        count,
        total,
    } = hint;
    let call = match query {
        Some(q) => format!("cx_recall(query={q:?}, {facet}=[{value:?}])"),
        None => format!("cx_recall({facet}=[{value:?}])"),
    };
    let qualifier = if facet == "tags" { "tagged " } else { "" };
    format!("# narrow: {call} - {count} of {total} results are {qualifier}{value}")
}
