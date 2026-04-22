use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::super::{HighlightStyle, RecallRow, kind_histogram, tag_histogram};
use super::normalise_bm25;
use crate::recall::{RecallRequest, RecallResult, RecallRouting};

/// Precomputed row-level state shared between the header, entries, and
/// trailer sections. Keeps each render helper's signature short and
/// ensures the short-id, BM25, and query-centred state is computed once.
pub(super) struct Layout<'a> {
    pub(super) rows: &'a [RecallRow],
    /// Whether the score column should be rendered. True only when the
    /// routing branch is `Search` and at least one row carries a raw
    /// BM25 score. Non-Search branches unconditionally suppress the
    /// column to avoid a `null` placeholder.
    pub(super) show_score: bool,
    /// Per-row normalised score in `[0.0, 1.0]`, parallel to `rows`.
    /// Empty when `show_score` is false.
    pub(super) norm_scores: Vec<f32>,
    /// Query term passed to `smart_snippet` for per-row snippet centring.
    /// `None` when the caller did not supply one (tag-/scope-only recall)
    /// or when the query is an empty string.
    pub(super) query: Option<&'a str>,
    /// Per-row snippet highlight style. [`HighlightStyle::Bracketed`] only
    /// when the routing branch is `Search` and `query` is populated, so
    /// matched terms render as `<<term>>`. Any other routing renders with
    /// [`HighlightStyle::None`] because the caller did not supply a query
    /// context the highlighter could meaningfully mark up.
    pub(super) highlight_style: HighlightStyle,
    /// Reference instant for relative-age formatting. Captured once by
    /// the public entry point so every row renders with a consistent
    /// `age:` value even if the underlying system clock drifts during
    /// the render call.
    pub(super) now: DateTime<Utc>,
    /// Outgoing-relation counts per row id, sourced from the
    /// [`RecallResult::relation_counts`] map produced by the single
    /// batched `count_relations_for` call in the recall capability.
    /// Borrowed so row comment rendering can do an `O(1)` lookup per
    /// row without cloning the map. Ids absent from the map carry zero
    /// outgoing edges and suppress the `rels:` annotation entirely.
    pub(super) relation_counts: &'a HashMap<Uuid, u32>,
    /// Per-result-set kind histogram, computed once at layout time so
    /// that the header and the trailer's drill-down advisory read the
    /// same data.
    pub(super) kind_hist: BTreeMap<String, usize>,
    /// Per-result-set tag histogram, computed once at layout time for
    /// the same reason as `kind_hist`. Each tag on each row contributes
    /// one bucket increment.
    pub(super) tag_hist: BTreeMap<String, usize>,
}

impl<'a> Layout<'a> {
    pub(super) fn new(
        rows: &'a [RecallRow],
        result: &'a RecallResult,
        request: &'a RecallRequest,
        now: DateTime<Utc>,
    ) -> Self {
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
