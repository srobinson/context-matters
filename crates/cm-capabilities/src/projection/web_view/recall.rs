use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::super::recall_view::{normalise_bm25, routing_explanation, search_tier_header_tag};
use super::super::{
    CountBucket, HighlightStyle, SNIPPET_MAX_BYTES, collapse_whitespace, count_buckets,
    count_desc_buckets, kind_histogram, relative_age, smart_snippet, tag_histogram,
};
use crate::recall::{RecallRequest, RecallResult, RecallRouting};

/// Top-of-response header for a recall result.
///
/// Mirrors the YAML `format_recall_view` header: surfaces the query,
/// routing branch, fallback tier, scope chain and hit counts,
/// histograms, and the token estimate after projection. `routing` and
/// `tier` are both strings so the frontend need not import the Rust
/// enum shapes; the values come straight out of the shared
/// `routing_explanation` and `search_tier_header_tag` helpers.
///
/// The web `cx_search` endpoint currently reuses this shape. Search
/// rows still carry `scope`, `kind`, `tags`, snippets, scores, and
/// histograms. Recall only fields degrade as follows: `tier` is `null`
/// unless recall performed its ancestor walk FTS fallback, and
/// `scope_chain` is empty for wide content search selectors
/// (`subtree`, `set`, `all`) because those requests do not produce an
/// ancestor walk.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebRecallHeader {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub routing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    pub candidates: u32,
    pub returned: usize,
    pub scope_chain: Vec<String>,
    /// Per-scope hit counts, preserved in the source ordering from the
    /// recall ancestor walk (most specific scope first, broadest last).
    /// Unlike the kind/tag histograms this is an ordered chain summary, not
    /// a frequency table, so it is NOT re-sorted by count; order matches
    /// `RecallResult::scope_hits` and the YAML `format_recall_view`
    /// rendering.
    pub scope_hits: Vec<CountBucket>,
    /// Per-kind counts across the returned rows, ordered by count
    /// descending (alphabetical tiebreak). Ordered buckets preserve count
    /// order on JSON transports while avoiding tuple-array output.
    pub kinds_histogram: Vec<CountBucket>,
    /// Per-tag counts across the returned rows, ordered by count descending.
    pub tags_histogram: Vec<CountBucket>,
    pub tokens: u32,
}

/// One row in a recall result, shaped for the cm-web UI.
///
/// Unlike `WebBrowseRow`, `scope` and `kind` are plain `String`s: the
/// recall view has no uniform-hoisting step because the scope chain
/// walk typically surfaces entries from multiple ancestors in the same
/// result, and the frontend wants the scope label visible per-row. The
/// BM25 `score` is normalised to `[0.0, 1.0]`, so the best row always
/// renders as `1.00` regardless of raw FTS5 range; non-search routings
/// leave `score` as `None`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebRecallRow {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    pub title: String,
    /// Smart snippet windowed around the first query match, with
    /// guillemet brackets when both routing is `Search` and the
    /// request supplied a non-empty query. Bracket style matches
    /// the YAML recall view exactly.
    pub snippet: String,
    pub age: String,
    pub scope: String,
    pub kind: String,
    pub tags: Vec<String>,
}

/// Full projection of a [`RecallResult`] for the cm-web HTTP API.
///
/// Structurally parallel to the YAML `format_recall_view` output.
/// `advisories` carries capability messages such as omitted-scope defaults.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebRecallView {
    pub header: WebRecallHeader,
    pub entries: Vec<WebRecallRow>,
    pub advisories: Vec<String>,
}

/// Project a [`RecallResult`] and its originating [`RecallRequest`]
/// into a [`WebRecallView`].
///
/// Takes the request because the header carries the `query:` field and
/// the snippet highlighting gate (`is_search && query.is_some()`) needs
/// the request to decide the style. Matches the arity of
/// `format_recall_view`. Captures `Utc::now()` once and delegates to
/// [`project_web_recall_at`] so tests can pin the age column.
pub fn project_web_recall(result: &RecallResult, request: &RecallRequest) -> WebRecallView {
    project_web_recall_at(result, request, Utc::now())
}

/// Deterministic variant of [`project_web_recall`]. Production callers
/// should prefer [`project_web_recall`].
pub fn project_web_recall_at(
    result: &RecallResult,
    request: &RecallRequest,
    now: DateTime<Utc>,
) -> WebRecallView {
    let rows = result.entries.as_slice();

    // Mirror `recall_view::Layout::new` verbatim. The `is_search` and
    // query predicates gate bracket highlighting, and the bracket rule
    // must stay in lock-step with the YAML formatter so the two views
    // cannot disagree about whether a given snippet should carry
    // guillemets. See the corresponding comment in recall_view.rs.
    let is_search = matches!(result.routing, RecallRouting::Search);
    let query = request.query.as_deref().filter(|q| !q.trim().is_empty());
    let highlight_style = if is_search && query.is_some() {
        HighlightStyle::Bracketed
    } else {
        HighlightStyle::None
    };

    let show_score = is_search && rows.iter().any(|r| r.score.is_some());
    let norm_scores: Vec<f32> = if show_score {
        let raws: Vec<f32> = rows.iter().map(|r| r.score.unwrap_or(0.0)).collect();
        normalise_bm25(&raws)
    } else {
        Vec::new()
    };

    let kinds_histogram = count_desc_buckets(kind_histogram(rows, |r| r.entry.kind.as_str()));
    let tags_histogram = count_desc_buckets(tag_histogram(rows, |r| {
        r.entry
            .meta
            .as_ref()
            .map(|m| m.tags.as_slice())
            .unwrap_or(&[])
    }));

    // Carry scope_hits verbatim in its source order (most specific scope
    // first, broadest last). This is an ordered ancestor-walk summary, not
    // a frequency histogram, so it must NOT be re-sorted: the array shape
    // matches both `RecallResult::scope_hits` and the YAML
    // `format_recall_view` rendering.
    let scope_hits = count_buckets(result.scope_hits.clone());

    let tier = if is_search {
        result
            .tier
            .and_then(search_tier_header_tag)
            .map(str::to_owned)
    } else {
        None
    };

    let header = WebRecallHeader {
        query: query.map(ToOwned::to_owned),
        routing: routing_explanation(&result.routing).0.to_owned(),
        tier,
        candidates: result.candidates_before_filter as u32,
        returned: rows.len(),
        scope_chain: result.scope_chain.clone(),
        scope_hits,
        kinds_histogram,
        tags_histogram,
        tokens: result.token_estimate,
    };

    let entries: Vec<WebRecallRow> = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let raw_snippet =
                smart_snippet(&row.entry.body, query, highlight_style, SNIPPET_MAX_BYTES);
            let snippet = collapse_whitespace(&raw_snippet);
            let tags = row
                .entry
                .meta
                .as_ref()
                .map(|m| m.tags.clone())
                .unwrap_or_default();
            let score = if show_score {
                Some(norm_scores[idx])
            } else {
                None
            };
            WebRecallRow {
                id: row.entry.id.to_string(),
                score,
                title: row.entry.title.clone(),
                snippet,
                age: relative_age(row.entry.updated_at, now),
                scope: row.entry.scope_path.as_str().to_owned(),
                kind: row.entry.kind.as_str().to_owned(),
                tags,
            }
        })
        .collect();

    WebRecallView {
        header,
        entries,
        advisories: result
            .advisories
            .iter()
            .map(|advisory| advisory.body().to_owned())
            .collect(),
    }
}
