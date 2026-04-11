//! `BrowseResult` YAML-text formatter for MCP response bodies.
//!
//! Consumed by `cx_browse` (via the wire-swap sub that lands the YAML
//! envelope) to replace the double-encoded JSON-in-text response shape
//! with a compact, agent-legible YAML view. The target shape is described
//! in `research/cx-response-payload-redesign-context-matters.md` §5.2.1.
//!
//! The formatter is pure text: no I/O, no allocations beyond the output
//! string and its temporaries. The only non-deterministic input is the
//! reference `now` used for relative-age rendering, which is captured
//! once at the entry point and injected into [`format_browse_view_at`]
//! so snapshot tests can pin the `age:` column.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use cm_core::{BrowseSort, Entry};

use super::{
    detect_id_collisions, hoist_uniform, kind_histogram, relative_age, scope_histogram, short_id,
    smart_snippet,
};
use crate::browse::{BrowseRequest, BrowseResult};

/// Maximum snippet width (bytes) shown per row in the browse view. Sized
/// to fit a prose-heavy line within one wide terminal row without wrap.
const SNIPPET_MAX_BYTES: usize = 200;

/// Default short-id length. Auto-extends to [`SHORT_ID_LEN_EXTENDED`] when
/// any two entries in the current result set share their first-8-byte
/// prefix. Matches the convention shared with the recall formatter.
const SHORT_ID_LEN: usize = 8;
const SHORT_ID_LEN_EXTENDED: usize = 12;

/// Render a [`BrowseResult`] as YAML-annotated text for the `cx_browse`
/// MCP response body. See the module docstring for the target shape.
///
/// Captures `Utc::now()` once for relative-age formatting and delegates
/// to [`format_browse_view_at`]. Use the `_at` variant from tests that
/// need the rendered `age:` column to be deterministic.
pub fn format_browse_view(result: &BrowseResult, request: &BrowseRequest) -> String {
    format_browse_view_at(result, request, Utc::now())
}

/// Deterministic variant of [`format_browse_view`] that takes an explicit
/// reference `now` for relative-age rendering. Production callers should
/// prefer [`format_browse_view`]; this entry point exists so snapshot
/// tests can pin the `age:` column without touching the system clock.
pub fn format_browse_view_at(
    result: &BrowseResult,
    request: &BrowseRequest,
    now: DateTime<Utc>,
) -> String {
    let entries = &result.entries;
    let hoists = Hoists {
        scope: hoist_uniform(entries, |e| e.scope_path.as_str().to_owned()),
        created_by: hoist_uniform(entries, |e| e.created_by.clone()),
    };

    let mut out = String::with_capacity(1024);
    out.push_str("---\n");
    render_header(&mut out, result, request, entries, &hoists);
    out.push('\n');
    render_entries(&mut out, entries, now, &hoists);
    render_pagination_hint(&mut out, result, request);
    out
}

/// Uniform-key hoists computed once so the header and each row agree on
/// which fields to print inline and which to omit. `Some(value)` means
/// every entry in the result set shares that value; `None` means the
/// field varies and rows must carry it themselves.
struct Hoists {
    scope: Option<String>,
    created_by: Option<String>,
}

fn render_header(
    out: &mut String,
    result: &BrowseResult,
    request: &BrowseRequest,
    entries: &[Entry],
    hoists: &Hoists,
) {
    if let Some(q) = reconstruct_query(request) {
        let _ = writeln!(out, "query: {q}");
    }
    let _ = writeln!(out, "sort: {}", sort_as_str(result.sort_used));
    let _ = writeln!(out, "total: {}", result.total);
    let _ = writeln!(out, "returned: {}", entries.len());

    if entries.is_empty() {
        return;
    }

    match &hoists.scope {
        Some(s) => {
            let _ = writeln!(out, "scope: {s}");
        }
        None => {
            let hist = scope_histogram(entries, |e| e.scope_path.as_str());
            let _ = writeln!(out, "scope: (mixed)  # {}", render_histogram(&hist));
        }
    }

    let kind_hist = kind_histogram(entries, |e| e.kind.as_str());
    if !kind_hist.is_empty() {
        let _ = writeln!(out, "kinds: {}", render_histogram(&kind_hist));
    }

    if let Some(c) = &hoists.created_by {
        let _ = writeln!(out, "created_by: {c}  # uniform");
    }
}

fn render_entries(out: &mut String, entries: &[Entry], now: DateTime<Utc>, hoists: &Hoists) {
    out.push_str("entries:\n");

    if entries.is_empty() {
        out.push_str("  []\n");
        return;
    }

    // Auto-extend the short id length when any two entries collide on
    // their first 8 bytes within this result set.
    let id_strings: Vec<String> = entries.iter().map(|e| e.id.to_string()).collect();
    let id_len = if detect_id_collisions(id_strings.iter().map(|s| s.as_str()), SHORT_ID_LEN) {
        SHORT_ID_LEN_EXTENDED
    } else {
        SHORT_ID_LEN
    };
    // Continuation lines align with the start of the title on line 1:
    //   "  - <id>  "  ⇒  2 (list indent) + 2 ("- ") + id_len + 2 (gap).
    let cont_indent = " ".repeat(4 + id_len + 2);

    for (entry, id_str) in entries.iter().zip(id_strings.iter()) {
        let sid = short_id(id_str, id_len);
        let _ = writeln!(out, "  - {sid}  {}", entry.title);

        let snippet = smart_snippet(&entry.body, None, SNIPPET_MAX_BYTES);
        let snippet_line = collapse_whitespace(&snippet);
        if !snippet_line.is_empty() {
            let _ = writeln!(out, "{cont_indent}{snippet_line}");
        }

        let comment = render_row_comment(entry, now, hoists);
        let _ = writeln!(out, "{cont_indent}# {comment}");
    }
}

fn render_row_comment(entry: &Entry, now: DateTime<Utc>, hoists: &Hoists) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(4);
    if hoists.scope.is_none() {
        parts.push(format!("scope: {}", entry.scope_path));
    }
    let tags: &[String] = entry
        .meta
        .as_ref()
        .map(|m| m.tags.as_slice())
        .unwrap_or(&[]);
    if !tags.is_empty() {
        parts.push(format!("tags: {}", tags.join(", ")));
    }
    parts.push(format!("age: {}", relative_age(entry.updated_at, now)));
    if hoists.created_by.is_none() {
        parts.push(format!("created_by: {}", entry.created_by));
    }
    parts.join("  ")
}

fn render_pagination_hint(out: &mut String, result: &BrowseResult, request: &BrowseRequest) {
    if !result.has_more {
        return;
    }
    let remaining = result.total.saturating_sub(result.entries.len() as u64);
    match &result.next_cursor {
        Some(cursor) => {
            let _ = writeln!(
                out,
                "\n# {remaining} more - cx_browse(cursor=\"{cursor}\", limit={limit}) to page",
                limit = request.limit
            );
        }
        None => {
            let _ = writeln!(out, "\n# {remaining} more - refine filters to narrow");
        }
    }
}

/// Reconstruct the `query:` header line from the `BrowseRequest` filters.
///
/// `cx_browse` has no free-text query, so we synthesize a flat
/// `key=value key=value ...` string from whichever filter fields are
/// set. Returns `None` when nothing is filtered, in which case the
/// formatter omits the `query:` line entirely.
fn reconstruct_query(req: &BrowseRequest) -> Option<String> {
    let mut parts: Vec<String> = Vec::with_capacity(5);
    if let Some(sp) = &req.scope_path {
        parts.push(format!("scope={sp}"));
    }
    if let Some(k) = &req.kind {
        parts.push(format!("kind={}", k.as_str()));
    }
    if let Some(t) = &req.tag {
        parts.push(format!("tag={t}"));
    }
    if let Some(c) = &req.created_by {
        parts.push(format!("created_by={c}"));
    }
    if req.include_superseded {
        parts.push("include_superseded=true".to_string());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Human-legible rendering for [`BrowseSort`] used in the `sort:` header
/// line. `Debug`/`serde` would give `Recent`/`recent`; we want the SQL
/// shape the sort resolves to, matching how `cx_recall` surfaces the
/// routing branch in its own header.
fn sort_as_str(sort: BrowseSort) -> &'static str {
    match sort {
        BrowseSort::Recent => "updated_at desc",
        BrowseSort::Oldest => "updated_at asc",
        BrowseSort::TitleAsc => "title asc",
        BrowseSort::TitleDesc => "title desc",
        BrowseSort::ScopeAsc => "scope asc",
        BrowseSort::ScopeDesc => "scope desc",
        BrowseSort::KindAsc => "kind asc",
        BrowseSort::KindDesc => "kind desc",
    }
}

/// Render a `BTreeMap<String, usize>` histogram as a comma-separated
/// `key=count` string, sorted by count descending with alphabetical
/// tiebreak. Matches the sort convention exercised by the
/// `kind_histogram_sorts_by_descending_count_then_alphabetical` test in
/// `aggregation.rs`.
fn render_histogram(hist: &BTreeMap<String, usize>) -> String {
    let mut sorted: Vec<(&String, &usize)> = hist.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let mut out = String::with_capacity(hist.len() * 16);
    for (i, (k, v)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(&mut out, "{k}={v}");
    }
    out
}

/// Collapse every run of ASCII whitespace in `s` to a single space and
/// trim leading and trailing whitespace. Used to keep smart-snippet
/// output on a single YAML line even when the source body contains
/// embedded newlines.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = true;
        } else {
            in_ws = false;
            out.push(ch);
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cm_core::EntryKind;

    #[test]
    fn sort_as_str_covers_every_variant() {
        assert_eq!(sort_as_str(BrowseSort::Recent), "updated_at desc");
        assert_eq!(sort_as_str(BrowseSort::Oldest), "updated_at asc");
        assert_eq!(sort_as_str(BrowseSort::TitleAsc), "title asc");
        assert_eq!(sort_as_str(BrowseSort::TitleDesc), "title desc");
        assert_eq!(sort_as_str(BrowseSort::ScopeAsc), "scope asc");
        assert_eq!(sort_as_str(BrowseSort::ScopeDesc), "scope desc");
        assert_eq!(sort_as_str(BrowseSort::KindAsc), "kind asc");
        assert_eq!(sort_as_str(BrowseSort::KindDesc), "kind desc");
    }

    #[test]
    fn reconstruct_query_joins_set_filters() {
        let mut req = BrowseRequest {
            limit: 50,
            ..Default::default()
        };
        // Empty filter → None.
        assert_eq!(reconstruct_query(&req), None);

        req.tag = Some("session-log".to_owned());
        assert_eq!(reconstruct_query(&req).as_deref(), Some("tag=session-log"));

        req.kind = Some(EntryKind::Observation);
        req.include_superseded = true;
        // Order matches the field order in the function body.
        assert_eq!(
            reconstruct_query(&req).as_deref(),
            Some("kind=observation tag=session-log include_superseded=true"),
        );
    }

    #[test]
    fn render_histogram_sorts_by_count_desc_then_alpha() {
        let mut hist = BTreeMap::new();
        hist.insert("fact".to_owned(), 2);
        hist.insert("decision".to_owned(), 2);
        hist.insert("lesson".to_owned(), 3);
        assert_eq!(render_histogram(&hist), "lesson=3, decision=2, fact=2");
    }

    #[test]
    fn collapse_whitespace_squashes_newlines_and_runs() {
        assert_eq!(collapse_whitespace("a\n\nb   c\n"), "a b c");
        assert_eq!(collapse_whitespace("  leading"), "leading");
        assert_eq!(collapse_whitespace("trailing\n"), "trailing");
        assert_eq!(collapse_whitespace(""), "");
    }
}
