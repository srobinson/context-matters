use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::super::browse_view::sort_as_str;
use super::super::{
    HighlightStyle, SNIPPET_MAX_BYTES, collapse_whitespace, hoist_uniform, kind_histogram,
    relative_age, smart_snippet, tag_histogram,
};
use super::histogram_to_u32;
use crate::browse::BrowseResult;
use crate::scope::{ScopeResolution, ScopeResolutionCandidate};

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
/// The full `id` is preserved so the client can pass it back to
/// `cx_get` / `cx_update`. `scope` and `kind` are hoisted to `None`
/// when the header carries the same value for every row.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebBrowseRow {
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

/// Scope inference metadata for a smart browse response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebScopeResolution {
    pub requested_scope: String,
    pub resolved_scope: String,
    pub scope_mode: String,
    pub confidence: String,
    pub candidates: Vec<WebScopeResolutionCandidate>,
    pub signals: Vec<String>,
}

/// One candidate considered by smart browse scope resolution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebScopeResolutionCandidate {
    pub scope: String,
    pub score: i32,
    pub matched: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<WebScopeResolution>,
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

    // Hoist uniform columns so rows drop values already carried by the header.
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

    let rows = entries
        .iter()
        .map(|e| {
            let raw_snippet = smart_snippet(&e.body, None, HighlightStyle::None, SNIPPET_MAX_BYTES);
            let snippet = collapse_whitespace(&raw_snippet);
            let tags = e.meta.as_ref().map(|m| m.tags.clone()).unwrap_or_default();
            WebBrowseRow {
                id: e.id.to_string(),
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
        advisory: result.advisory.clone(),
        resolution: if result.include_resolution {
            result.resolution.as_ref().map(project_scope_resolution)
        } else {
            None
        },
        entries: rows,
        next_cursor: result.next_cursor.clone(),
        has_more: result.has_more,
    }
}

fn project_scope_resolution(resolution: &ScopeResolution) -> WebScopeResolution {
    WebScopeResolution {
        requested_scope: resolution.requested_scope.clone(),
        resolved_scope: resolution.resolved_scope.as_str().to_owned(),
        scope_mode: resolution.scope_mode.as_str().to_owned(),
        confidence: resolution.confidence.as_str().to_owned(),
        candidates: resolution
            .candidates
            .iter()
            .map(project_scope_resolution_candidate)
            .collect(),
        signals: resolution.signals.clone(),
    }
}

fn project_scope_resolution_candidate(
    candidate: &ScopeResolutionCandidate,
) -> WebScopeResolutionCandidate {
    WebScopeResolutionCandidate {
        scope: candidate.scope.as_str().to_owned(),
        score: candidate.score,
        matched: candidate.matched.clone(),
    }
}
