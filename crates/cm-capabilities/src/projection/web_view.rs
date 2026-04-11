//! Typed projection views for the cm-web HTTP API.
//!
//! Consumed by the cm-web Curator UI. Mirrors the information surfaced
//! by the YAML `format_browse_view` and `format_recall_view` formatters
//! as serialisable structs, so the web front-end renders the same short
//! ids, smart snippets, relative ages, and hoisted headers that the MCP
//! adapter shows. HTTP wiring, ts-rs regeneration, and frontend
//! consumption land in the follow-on issues ALP-1752 / 1753 / 1754.
//!
//! This module is the **data-shape** layer only. It does not touch the
//! store, the capability, or the HTTP surface. Each `project_web_*`
//! function is a pure transformation from the capability result
//! (and its originating request, where one is needed) to a view
//! struct.
//!
//! Every shared computation — short-id collision detection, snippet
//! windowing, histogram aggregation, uniform-key hoisting, BM25
//! normalisation, routing/tier tagging — is delegated to the existing
//! helpers in the sibling projection modules. The YAML and web views
//! cannot drift on any of these because they read from the same source
//! of truth. See the DRY notes on [`super::browse_view::sort_as_str`],
//! [`super::recall_view::routing_explanation`], and
//! [`super::recall_view::search_tier_header_tag`] for the three helpers
//! that were promoted to `pub(crate)` specifically so this module could
//! reuse them verbatim.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::browse_view::sort_as_str;
use super::recall_view::{normalise_bm25, routing_explanation, search_tier_header_tag};
use super::{
    HighlightStyle, SHORT_ID_LEN, SHORT_ID_LEN_EXTENDED, SNIPPET_MAX_BYTES, collapse_whitespace,
    detect_id_collisions, hoist_uniform, kind_histogram, relative_age, short_id, smart_snippet,
    tag_histogram,
};
use crate::browse::BrowseResult;
use crate::recall::{RecallRequest, RecallResult, RecallRouting};

// ── Browse view ──────────────────────────────────────────────────

/// Top-of-response header for a browse result.
///
/// Fields marked "hoisted" collapse a uniform column out of every row
/// into the header: if every row in the result set shares the same
/// `scope`, `kind`, or `created_by`, the header carries the value once
/// and each row drops it. When the column varies, the header field is
/// `None` and rows carry their own value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebBrowseHeader {
    /// Human-legible SQL form of the sort order actually applied,
    /// e.g. `"updated_at desc"`. Matches the YAML `sort:` header.
    pub sort_used: String,
    /// Total number of entries matching the request filters across
    /// the full result set, before pagination.
    pub total: u64,
    /// Number of entries returned in the current page.
    pub returned: usize,
    /// Hoisted uniform scope path. `Some` only when every entry shares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Hoisted uniform kind. `Some` only when every entry shares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Hoisted uniform creator. `Some` only when every entry shares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// Frequency of each entry kind present in the returned slice.
    pub kinds_histogram: BTreeMap<String, u32>,
    /// Frequency of each tag occurrence across the returned slice.
    pub tags_histogram: BTreeMap<String, u32>,
}

/// One row in a browse result, shaped for the cm-web UI.
///
/// The `short_id` field is the 8-char (12-char on collision) prefix
/// used for visual identification; the full `id` is preserved so the
/// client can pass it back to `cx_get` / `cx_update`. `scope` and
/// `kind` are hoisted to `None` when the header carries the same
/// value for every row.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebBrowseRow {
    pub short_id: String,
    pub id: String,
    pub title: String,
    /// Smart snippet (frontmatter/heading stripped, windowed to
    /// [`SNIPPET_MAX_BYTES`], whitespace collapsed). Browse has no
    /// query context, so no bracket highlighting is applied.
    pub snippet: String,
    pub age: String,
    /// Row-local scope. `None` when the header hoisted a uniform scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Row-local kind. `None` when the header hoisted a uniform kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub tags: Vec<String>,
}

/// Full projection of a [`BrowseResult`] for the cm-web HTTP API.
///
/// Structurally parallel to the YAML `format_browse_view` output: same
/// header fields, same row shape, same pagination hint. Frontends that
/// render this view see the exact mental model the MCP adapter shows.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebBrowseView {
    pub header: WebBrowseHeader,
    pub entries: Vec<WebBrowseRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Project a [`BrowseResult`] into a [`WebBrowseView`].
///
/// Captures `Utc::now()` once for relative-age formatting and delegates
/// to [`project_web_browse_at`] so snapshot-style tests can pin the
/// `age` field deterministically.
///
/// Does not take a `BrowseRequest`: no field on [`WebBrowseHeader`] needs
/// the request, and the cm-web frontend already knows the filter
/// parameters from its own query string. Introducing a
/// `BrowseQueryContext` wrapper purely to satisfy the spec signature
/// would duplicate the fields of [`crate::browse::BrowseRequest`] for
/// no runtime benefit, which the CLAUDE.md DRY invariant forbids.
pub fn project_web_browse(result: &BrowseResult) -> WebBrowseView {
    project_web_browse_at(result, Utc::now())
}

/// Deterministic variant of [`project_web_browse`] that takes an
/// explicit reference `now` for relative-age rendering. Production
/// callers should prefer [`project_web_browse`].
pub fn project_web_browse_at(result: &BrowseResult, now: DateTime<Utc>) -> WebBrowseView {
    let entries = result.entries.as_slice();

    // Hoist uniform header columns so rows drop them. Matches the three
    // hoists in `format_browse_view` exactly: scope, created_by, and
    // (new for the web view) kind. The YAML view does not hoist kind
    // because the `kinds:` histogram in the header already shows the
    // distribution; the web view hoists it because the frontend column
    // presentation benefits from the null-signal.
    let hoisted_scope = hoist_uniform(entries, |e| e.scope_path.as_str().to_owned());
    let hoisted_kind = hoist_uniform(entries, |e| e.kind.as_str().to_owned());
    let hoisted_created_by = hoist_uniform(entries, |e| e.created_by.clone());

    let kinds_histogram = histogram_to_u32(kind_histogram(entries, |e| e.kind.as_str()));
    let tags_histogram = histogram_to_u32(tag_histogram(entries, |e| {
        e.meta.as_ref().map(|m| m.tags.as_slice()).unwrap_or(&[])
    }));

    let header = WebBrowseHeader {
        sort_used: sort_as_str(result.sort_used).to_owned(),
        total: result.total,
        returned: entries.len(),
        scope: hoisted_scope.clone(),
        kind: hoisted_kind.clone(),
        created_by: hoisted_created_by,
        kinds_histogram,
        tags_histogram,
    };

    let id_strings: Vec<String> = entries.iter().map(|e| e.id.to_string()).collect();
    let id_len = resolve_id_len(&id_strings);

    let rows = entries
        .iter()
        .zip(id_strings.iter())
        .map(|(e, id_str)| {
            let raw_snippet = smart_snippet(&e.body, None, HighlightStyle::None, SNIPPET_MAX_BYTES);
            let snippet = collapse_whitespace(&raw_snippet);
            let tags = e.meta.as_ref().map(|m| m.tags.clone()).unwrap_or_default();
            WebBrowseRow {
                short_id: short_id(id_str, id_len).to_owned(),
                id: id_str.clone(),
                title: e.title.clone(),
                snippet,
                age: relative_age(e.updated_at, now),
                scope: if hoisted_scope.is_none() {
                    Some(e.scope_path.as_str().to_owned())
                } else {
                    None
                },
                kind: if hoisted_kind.is_none() {
                    Some(e.kind.as_str().to_owned())
                } else {
                    None
                },
                tags,
            }
        })
        .collect();

    WebBrowseView {
        header,
        entries: rows,
        next_cursor: result.next_cursor.clone(),
        has_more: result.has_more,
    }
}

// ── Recall view ──────────────────────────────────────────────────

/// Top-of-response header for a recall result.
///
/// Mirrors the YAML `format_recall_view` header: surfaces the query,
/// routing branch, fallback tier, scope chain and hit counts,
/// histograms, and the post-projection token estimate. `routing` and
/// `tier` are both strings so the frontend need not import the Rust
/// enum shapes — the values come straight out of
/// [`super::recall_view::routing_explanation`] and
/// [`super::recall_view::search_tier_header_tag`], sharing the source
/// of truth with the YAML renderer.
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
    pub scope_hits: BTreeMap<String, usize>,
    pub kinds_histogram: BTreeMap<String, u32>,
    pub tags_histogram: BTreeMap<String, u32>,
    pub tokens: u32,
}

/// One row in a recall result, shaped for the cm-web UI.
///
/// Unlike [`WebBrowseRow`], `scope` and `kind` are plain `String`s: the
/// recall view has no uniform-hoisting step because the scope chain
/// walk typically surfaces entries from multiple ancestors in the same
/// result, and the frontend wants the scope label visible per-row. The
/// BM25 `score` is normalised to `[0.0, 1.0]` via
/// [`super::recall_view::normalise_bm25`], so the best row always
/// renders as `1.00` regardless of raw FTS5 range; non-search routings
/// leave `score` as `None`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebRecallRow {
    pub short_id: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    pub title: String,
    /// Smart snippet windowed around the first query match, with
    /// `«term»` brackets when both routing is `Search` and the
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
/// `advisories` is a forward-compatible slot for the dominance /
/// drill-down hints landing in ALP-1758; this issue leaves it empty.
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

    let id_strings: Vec<String> = rows.iter().map(|r| r.entry.id.to_string()).collect();
    let id_len = resolve_id_len(&id_strings);

    let show_score = is_search && rows.iter().any(|r| r.score.is_some());
    let norm_scores: Vec<f32> = if show_score {
        let raws: Vec<f32> = rows.iter().map(|r| r.score.unwrap_or(0.0)).collect();
        normalise_bm25(&raws)
    } else {
        Vec::new()
    };

    let kinds_histogram = histogram_to_u32(kind_histogram(rows, |r| r.entry.kind.as_str()));
    let tags_histogram = histogram_to_u32(tag_histogram(rows, |r| {
        r.entry
            .meta
            .as_ref()
            .map(|m| m.tags.as_slice())
            .unwrap_or(&[])
    }));

    // Preserve the caller-provided ordering of scope_hits (which is
    // ordered most-specific-first) by collecting into a BTreeMap, which
    // will re-sort alphabetically. The YAML formatter renders
    // scope_hits as an insertion-ordered list; the web view
    // intentionally sorts alphabetically so the JSON surface is
    // deterministic and diff-friendly.
    let scope_hits: BTreeMap<String, usize> = result.scope_hits.iter().cloned().collect();

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
        .zip(id_strings.iter())
        .map(|((idx, row), id_str)| {
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
                short_id: short_id(id_str, id_len).to_owned(),
                id: id_str.clone(),
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
        // Reserved for faceted drill-down / dominance hints in ALP-1758.
        // Emitting an empty Vec rather than Option keeps the shape
        // stable for the ts-rs export in ALP-1753 and lets the frontend
        // render a deterministic (possibly empty) list.
        advisories: Vec::new(),
    }
}

// ── Private helpers ──────────────────────────────────────────────

/// Pick the active short-id length for a result slice.
///
/// Keeps the browse/recall/web views in lock-step on the 8-vs-12 byte
/// rule without each caller re-implementing the branch. Returns 8 on
/// the empty slice because no collision is possible there.
fn resolve_id_len(id_strings: &[String]) -> usize {
    if detect_id_collisions(id_strings.iter().map(String::as_str), SHORT_ID_LEN) {
        SHORT_ID_LEN_EXTENDED
    } else {
        SHORT_ID_LEN
    }
}

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
