use std::fmt::Write as _;

use super::super::{fmt_with_commas, render_histogram};
use super::layout::Layout;
use super::{routing_explanation, search_tier_header_tag};
use crate::recall::{RecallRequest, RecallResult, RecallRouting};

pub(super) fn render_header(
    out: &mut String,
    result: &RecallResult,
    request: &RecallRequest,
    layout: &Layout,
) {
    if let Some(q) = layout.query {
        let _ = writeln!(out, "query: {q}");
    }
    let (routing_str, routing_explain) = routing_explanation(&result.routing);
    let tier_suffix = if matches!(result.routing, RecallRouting::Search) {
        result
            .tier
            .and_then(search_tier_header_tag)
            .map(|tag| format!(", tier: {tag}"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let _ = writeln!(
        out,
        "routing: {routing_str}  # {routing_explain}{tier_suffix}"
    );
    let _ = writeln!(
        out,
        "candidates: {before} -> {shown} shown",
        before = result.candidates_before_filter,
        shown = result.entries.len(),
    );
    if !result.scope_chain.is_empty() {
        let _ = writeln!(out, "scope_chain: [{}]", result.scope_chain.join(", "));
    }
    if !result.scope_hits.is_empty() {
        let rendered: Vec<String> = result
            .scope_hits
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let _ = writeln!(out, "scope_hits: {}", rendered.join(", "));
    }
    if !result.entries.is_empty() {
        // Histograms are precomputed once on `Layout` so the header
        // and the trailer's drill-down advisory read the same data.
        if !layout.kind_hist.is_empty() {
            let _ = writeln!(out, "kinds: {}", render_histogram(&layout.kind_hist));
        }
        // Per-tag histogram mirrors `kinds:` so agents can scan tag
        // density on the result set without paging through rows.
        // Falls through when no row carries any tag so the header
        // does not sprout an empty `tags:` line on tag-free stores.
        if !layout.tag_hist.is_empty() {
            let _ = writeln!(out, "tags: {}", render_histogram(&layout.tag_hist));
        }
    }
    match request.max_tokens {
        Some(budget) => {
            let _ = writeln!(
                out,
                "tokens: {used} of {budget} budget",
                used = fmt_with_commas(result.token_estimate),
                budget = fmt_with_commas(budget),
            );
        }
        None => {
            let _ = writeln!(out, "tokens: {}", fmt_with_commas(result.token_estimate));
        }
    }
}
