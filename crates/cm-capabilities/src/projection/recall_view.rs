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

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use cm_core::Entry;
use uuid::Uuid;

use super::{
    DrillDownHint, HighlightStyle, RecallRow, SHORT_ID_LEN, SHORT_ID_LEN_EXTENDED,
    SNIPPET_MAX_BYTES, collapse_whitespace, compute_dedup_hints, compute_drill_down_hint,
    detect_id_collisions, estimate_tokens, fmt_with_commas, kind_histogram, relative_age,
    render_histogram, short_id, smart_snippet, tag_histogram,
};
use crate::recall::{RecallRequest, RecallResult, RecallRouting, SearchTier};

/// Per-row body size above which the formatter emits a `cx_get(...)` hint
/// suggesting the caller fetch full content separately. Tuned to slightly
/// below the recall-default per-row snippet budget.
const TOKEN_HINT_THRESHOLD: u32 = 1024;

/// Maximum number of short ids the `cx_get(...)` hint lists explicitly
/// before appending `...`. Keeps the trailer bounded on large result sets.
const TOKEN_HINT_MAX_IDS: usize = 6;

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

/// Precomputed row-level state shared between the header, entries, and
/// trailer sections. Keeps each render helper's signature short and
/// ensures the short-id, BM25, and query-centred state is computed once.
struct Layout<'a> {
    rows: &'a [RecallRow],
    /// Full UUID hex for each row, parallel to `rows`. Owned so the
    /// downstream `short_id` slicing can borrow these strings for the
    /// duration of the render.
    id_strings: Vec<String>,
    /// Active short-id length (8 by default; 12 when any two entries
    /// in the current slice collide on their first 8 bytes).
    id_len: usize,
    /// Whether the score column should be rendered. True only when the
    /// routing branch is `Search` and at least one row carries a raw
    /// BM25 score. Non-Search branches unconditionally suppress the
    /// column to avoid a `null` placeholder.
    show_score: bool,
    /// Per-row normalised score in `[0.0, 1.0]`, parallel to `rows`.
    /// Empty when `show_score` is false.
    norm_scores: Vec<f32>,
    /// Query term passed to `smart_snippet` for per-row snippet centring.
    /// `None` when the caller did not supply one (tag-/scope-only recall)
    /// or when the query is an empty string.
    query: Option<&'a str>,
    /// Per-row snippet highlight style. [`HighlightStyle::Bracketed`] only
    /// when the routing branch is `Search` and `query` is populated, so
    /// matched terms render as `«term»`. Any other routing (browse
    /// fallback, tag/scope walk) renders with [`HighlightStyle::None`]
    /// because the caller did not supply a query context the highlighter
    /// could meaningfully mark up.
    highlight_style: HighlightStyle,
    /// Reference instant for relative-age formatting. Captured once by
    /// the public entry point so every row renders with a consistent
    /// `age:` value even if the underlying system clock drifts during
    /// the render call.
    now: DateTime<Utc>,
    /// Outgoing-relation counts per row id, sourced from the
    /// [`RecallResult::relation_counts`] map produced by the single
    /// batched `count_relations_for` call in the recall capability.
    /// Borrowed so [`render_row_comment`] can do an `O(1)` lookup per
    /// row without cloning the map. Ids absent from the map carry zero
    /// outgoing edges and suppress the `rels:` annotation entirely.
    relation_counts: &'a HashMap<Uuid, u32>,
    /// Per-result-set kind histogram, computed once at layout time so
    /// that [`render_header`] and [`render_trailers`] read the same
    /// `BTreeMap` instead of recomputing it. The trailer needs the
    /// histogram to feed [`compute_drill_down_hint`] without paging
    /// over `rows` a second time.
    kind_hist: BTreeMap<String, usize>,
    /// Per-result-set tag histogram, computed once at layout time for
    /// the same reason as `kind_hist`. Each tag on each row contributes
    /// one bucket increment, mirroring the histogram the header line
    /// has always rendered.
    tag_hist: BTreeMap<String, usize>,
}

impl<'a> Layout<'a> {
    fn new(
        rows: &'a [RecallRow],
        result: &'a RecallResult,
        request: &'a RecallRequest,
        now: DateTime<Utc>,
    ) -> Self {
        let id_strings: Vec<String> = rows.iter().map(|r| r.entry.id.to_string()).collect();
        let id_len = if detect_id_collisions(id_strings.iter().map(|s| s.as_str()), SHORT_ID_LEN) {
            SHORT_ID_LEN_EXTENDED
        } else {
            SHORT_ID_LEN
        };
        let is_search = matches!(result.routing, RecallRouting::Search);
        let show_score = is_search && rows.iter().any(|r| r.score.is_some());
        let norm_scores = if show_score {
            let raws: Vec<f32> = rows.iter().map(|r| r.score.unwrap_or(0.0)).collect();
            normalise_bm25(&raws)
        } else {
            Vec::new()
        };
        let query = request.query.as_deref().filter(|q| !q.trim().is_empty());
        let highlight_style = if is_search && query.is_some() {
            HighlightStyle::Bracketed
        } else {
            HighlightStyle::None
        };
        let kind_hist = kind_histogram(rows, |row| row.entry.kind.as_str());
        let tag_hist = tag_histogram(rows, |row| {
            row.entry
                .meta
                .as_ref()
                .map(|m| m.tags.as_slice())
                .unwrap_or(&[])
        });
        Self {
            rows,
            id_strings,
            id_len,
            show_score,
            norm_scores,
            query,
            highlight_style,
            now,
            relation_counts: &result.relation_counts,
            kind_hist,
            tag_hist,
        }
    }
}

fn render_header(
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

fn render_entries(out: &mut String, layout: &Layout) {
    out.push_str("entries:\n");
    if layout.rows.is_empty() {
        out.push_str("  []\n");
        return;
    }

    // Continuation lines align with the start of the title column on
    // line 1:
    //   "  - <id>  Title"              -> 4 (list indent + "- ") + id_len + 2 (gap)
    //   "  - <id>  X.XX  Title"        -> 4 + id_len + 2 + 4 (score) + 2 (gap)
    let cont_indent = if layout.show_score {
        " ".repeat(4 + layout.id_len + 2 + 4 + 2)
    } else {
        " ".repeat(4 + layout.id_len + 2)
    };

    // Intra-response dedup: first occurrence of each content-hash
    // prefix is the leader; every later row whose prefix collides
    // with a leader gets a `dup_of: <short leader id>` annotation in
    // its trailing comment. Computed once per render pass.
    let entries: Vec<&Entry> = layout.rows.iter().map(|r| &r.entry).collect();
    let dedup = compute_dedup_hints(&entries);

    for (i, (row, id_str)) in layout.rows.iter().zip(layout.id_strings.iter()).enumerate() {
        let sid = short_id(id_str, layout.id_len);
        if layout.show_score {
            let s = layout.norm_scores.get(i).copied().unwrap_or(0.0);
            let _ = writeln!(out, "  - {sid}  {s:.2}  {}", row.entry.title);
        } else {
            let _ = writeln!(out, "  - {sid}  {}", row.entry.title);
        }

        let snippet = smart_snippet(
            &row.entry.body,
            layout.query,
            layout.highlight_style,
            SNIPPET_MAX_BYTES,
        );
        let snippet_line = collapse_whitespace(&snippet);
        if !snippet_line.is_empty() {
            let _ = writeln!(out, "{cont_indent}{snippet_line}");
        }

        let dup_of = dedup
            .get(&row.entry.id)
            .map(|leader_uuid| short_id(&leader_uuid.to_string(), layout.id_len).to_owned());
        let rels = layout
            .relation_counts
            .get(&row.entry.id)
            .copied()
            .unwrap_or(0);
        let comment = render_row_comment(&row.entry, layout.now, dup_of.as_deref(), rels);
        let _ = writeln!(out, "{cont_indent}# {comment}");
    }
}

fn render_row_comment(
    entry: &Entry,
    now: DateTime<Utc>,
    dup_of: Option<&str>,
    rels: u32,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(6);
    parts.push(format!("scope: {}", entry.scope_path));
    parts.push(format!("kind: {}", entry.kind.as_str()));
    let tags: &[String] = entry
        .meta
        .as_ref()
        .map(|m| m.tags.as_slice())
        .unwrap_or(&[]);
    if !tags.is_empty() {
        parts.push(format!("tags: {}", tags.join(", ")));
    }
    parts.push(format!("age: {}", relative_age(entry.updated_at, now)));
    if let Some(dup) = dup_of {
        parts.push(format!("dup_of: {dup}"));
    }
    if rels > 0 {
        parts.push(format!("rels: {rels}"));
    }
    parts.join("  ")
}

fn render_trailers(out: &mut String, result: &RecallResult, layout: &Layout) {
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
    // ≥60% of the result set, append a one-line `narrow:` hint that
    // shows the caller the exact `cx_recall(...)` shape to re-issue
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

    // Two-phase hint: list short ids of rows whose body alone exceeds
    // `TOKEN_HINT_THRESHOLD`, encouraging the caller to fetch full
    // content via `cx_get(id=...)` rather than re-render the snippet.
    let big_ids: Vec<&str> = layout
        .rows
        .iter()
        .zip(layout.id_strings.iter())
        .filter(|(row, _)| estimate_tokens(&row.entry.body) > TOKEN_HINT_THRESHOLD)
        .map(|(_, id)| short_id(id.as_str(), layout.id_len))
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

/// Min-max normalise raw BM25 scores into `[0.0, 1.0]` with inversion so
/// that a higher normalised value corresponds to a better match.
///
/// `cm-store` surfaces raw SQLite `bm25()` output on `Search`-routed
/// recall rows: floating-point values ≤ 0 where lower (more negative)
/// means a better match. This function applies
///
/// ```text
///     norm = 1.0 - (raw - min) / (max - min)
/// ```
///
/// so the best (most-negative) raw becomes `1.00` and the worst becomes
/// `0.00`. When every raw score is equal (including the single-row case)
/// the formula's divisor is zero; this function emits `1.00` for every
/// row in that case rather than returning NaN.
pub fn normalise_bm25(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let min = scores.iter().copied().fold(f32::INFINITY, f32::min);
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;
    if range.abs() < f32::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores
        .iter()
        .map(|&raw| 1.0 - (raw - min) / range)
        .collect()
}

/// Header rendering for the `routing:` line: `(tag, one-line explanation)`.
///
/// The tag matches the serde `rename_all = "snake_case"` rendering of the
/// enum so callers searching by routing name find the same string in
/// the text envelope and the structured log channel.
///
/// Crate-visible so [`crate::projection::web_view`] can pick the same
/// tag for `WebRecallHeader::routing` without re-matching every enum
/// variant. Only the `.0` tag is needed there; the explanation text is
/// YAML-specific and stays in the trailer.
pub(crate) fn routing_explanation(routing: &RecallRouting) -> (&'static str, &'static str) {
    match routing {
        RecallRouting::Search => ("search", "FTS5 ranking"),
        RecallRouting::TagScopeWalk => ("tag_scope_walk", "tag + ancestor walk"),
        RecallRouting::ScopeResolve => ("scope_resolve", "recent entries in scope"),
        RecallRouting::BrowseFallback => ("browse_fallback", "recency fallback"),
    }
}

/// Trailer rendering for the `# routing: ...` advisory line: `(tag, next-step hint)`.
///
/// Re-uses the `routing_explanation` tag so the header and trailer agree
/// on the canonical snake_case name. The advisory tells the caller how
/// to broaden or narrow the query if the current result set is unhelpful.
fn routing_advice(routing: &RecallRouting) -> (&'static str, &'static str) {
    let tag = routing_explanation(routing).0;
    let advice = match routing {
        RecallRouting::Search => {
            "re-query with OR between synonyms or prefix match (term*) for more breadth"
        }
        RecallRouting::TagScopeWalk => {
            "no FTS query supplied; try a free-text query, broader tag, or higher scope"
        }
        RecallRouting::ScopeResolve => {
            "returning most-recent entries in scope; add a query or tag to narrow"
        }
        RecallRouting::BrowseFallback => {
            "no FTS match in scope; falling back to most-recent entries anywhere"
        }
    };
    (tag, advice)
}

/// Header suffix tag for the cascade's winning [`SearchTier`]. Returns
/// the snake_case name for the three winning tiers and `None` for
/// [`SearchTier::None`], so the header stays clean when all three
/// tiers were exhausted (the empty-result trailer covers that case).
///
/// Crate-visible so [`crate::projection::web_view`] can project the
/// same tag into `WebRecallHeader::tier`. Shared so the YAML and web
/// views cannot drift on the stringified tier name.
pub(crate) fn search_tier_header_tag(tier: SearchTier) -> Option<&'static str> {
    match tier {
        SearchTier::Exact => Some("exact"),
        SearchTier::Prefix => Some("prefix"),
        SearchTier::SplitOr => Some("split_or"),
        SearchTier::None => None,
    }
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

/// Trailer advisory line for the cascade's winning [`SearchTier`].
/// Fires only on `Prefix` and `SplitOr`: those tiers produced a result
/// set from a query shape the caller did not write, so the LLM needs
/// to be told about the rewrite to learn what succeeded. `Exact` is
/// the happy path (silent); `None` is covered by the `no matches`
/// trailer, so a tier advisory there would be redundant noise.
fn search_tier_trailer(tier: SearchTier) -> Option<String> {
    let (tag, advice) = match tier {
        SearchTier::Prefix => (
            "prefix",
            "original query had zero exact hits, tried prefix match",
        ),
        SearchTier::SplitOr => (
            "split_or",
            "original query had zero prefix hits, OR-joined tokens",
        ),
        SearchTier::Exact | SearchTier::None => return None,
    };
    Some(format!("# tier: {tag} - {advice}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_bm25_inverts_negative_raw_scores() {
        // Raw BM25 values as surfaced by `cm-store` on a Search-routed
        // recall: all negative, lower = better. Expected normalisation
        // after inversion: the most-negative raw maps to 1.00 (best),
        // the least-negative maps to 0.00 (worst).
        //
        // Formula: norm = 1.0 - (raw - min) / (max - min)
        //   min=-3.47, max=-0.88, range=2.59
        //   -3.47 -> 1.0 - 0.00 / 2.59 = 1.00
        //   -1.12 -> 1.0 - 2.35 / 2.59 ≈ 0.09
        //   -0.88 -> 1.0 - 2.59 / 2.59 = 0.00
        //
        // NOTE: ALP-1731's spec example listed the non-inverted values
        // `[0.00, 0.91, 1.00]`, which would map the best match to 0.00.
        // That is a spec-authoring mistake; the formula and the
        // store-side "lower = better" convention require inversion.
        let raws = [-3.47_f32, -1.12, -0.88];
        let norms = normalise_bm25(&raws);
        assert_eq!(round2(norms[0]), 1.00);
        assert_eq!(round2(norms[1]), 0.09);
        assert_eq!(round2(norms[2]), 0.00);
    }

    #[test]
    fn normalise_bm25_uniform_scores_collapse_to_one() {
        // When every raw score is equal, the divisor is zero. The
        // function emits 1.00 for every row rather than returning NaN.
        assert_eq!(normalise_bm25(&[-2.5, -2.5, -2.5]), vec![1.0, 1.0, 1.0]);
        // Single-row slices also hit the uniform branch.
        assert_eq!(normalise_bm25(&[-1.0]), vec![1.0]);
    }

    #[test]
    fn normalise_bm25_empty_is_empty() {
        assert!(normalise_bm25(&[]).is_empty());
    }

    #[test]
    fn routing_explanation_covers_every_variant() {
        assert_eq!(routing_explanation(&RecallRouting::Search).0, "search");
        assert_eq!(
            routing_explanation(&RecallRouting::TagScopeWalk).0,
            "tag_scope_walk",
        );
        assert_eq!(
            routing_explanation(&RecallRouting::ScopeResolve).0,
            "scope_resolve",
        );
        assert_eq!(
            routing_explanation(&RecallRouting::BrowseFallback).0,
            "browse_fallback",
        );
        // Every explanation is non-empty so the header `#` comment
        // never renders as a dangling prefix.
        for routing in [
            RecallRouting::Search,
            RecallRouting::TagScopeWalk,
            RecallRouting::ScopeResolve,
            RecallRouting::BrowseFallback,
        ] {
            let (tag, explain) = routing_explanation(&routing);
            assert!(!tag.is_empty() && !explain.is_empty());
        }
    }

    #[test]
    fn routing_advice_tag_matches_routing_explanation_tag() {
        for routing in [
            RecallRouting::Search,
            RecallRouting::TagScopeWalk,
            RecallRouting::ScopeResolve,
            RecallRouting::BrowseFallback,
        ] {
            assert_eq!(
                routing_explanation(&routing).0,
                routing_advice(&routing).0,
                "routing tag must agree between header and trailer for {routing:?}",
            );
            assert!(!routing_advice(&routing).1.is_empty());
        }
    }

    #[test]
    fn search_tier_header_tag_hides_none_variant() {
        // Exact / Prefix / SplitOr surface in the header; SearchTier::None
        // returns None so the header omits the tier suffix when the
        // cascade exhausted every tier without a hit.
        assert_eq!(search_tier_header_tag(SearchTier::Exact), Some("exact"));
        assert_eq!(search_tier_header_tag(SearchTier::Prefix), Some("prefix"));
        assert_eq!(
            search_tier_header_tag(SearchTier::SplitOr),
            Some("split_or"),
        );
        assert_eq!(search_tier_header_tag(SearchTier::None), None);
    }

    #[test]
    fn search_tier_trailer_fires_only_on_fallback_tiers() {
        // Exact is the happy path (no advisory). None is already covered
        // by the `no matches` trailer, so duplicating it would be noise.
        assert!(search_tier_trailer(SearchTier::Exact).is_none());
        assert!(search_tier_trailer(SearchTier::None).is_none());
        // Prefix and SplitOr emit a trailing advisory describing the
        // rewrite so the LLM learns which fallback succeeded.
        let prefix = search_tier_trailer(SearchTier::Prefix).expect("prefix emits");
        assert!(
            prefix.starts_with("# tier: prefix - "),
            "prefix advisory shape: {prefix}",
        );
        assert!(
            prefix.contains("zero exact hits"),
            "prefix advisory text: {prefix}",
        );
        let split_or = search_tier_trailer(SearchTier::SplitOr).expect("split_or emits");
        assert!(
            split_or.starts_with("# tier: split_or - "),
            "split_or advisory shape: {split_or}",
        );
        assert!(
            split_or.contains("OR-joined"),
            "split_or advisory text: {split_or}",
        );
    }

    #[test]
    fn fmt_with_commas_inserts_thousands_separators() {
        // Canonical behaviour is tested in the aggregation module; this
        // test only guards the recall-side call sites against a regression
        // that would stop accepting `u32` through the `impl Into<u64>`
        // signature when the helper moved out of this file.
        assert_eq!(fmt_with_commas(0_u32), "0");
        assert_eq!(fmt_with_commas(3_420_u32), "3,420");
    }

    /// Round to two decimal places for assertion-friendly comparisons
    /// against the normalised BM25 output.
    fn round2(x: f32) -> f32 {
        (x * 100.0).round() / 100.0
    }
}
