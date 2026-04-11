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

use std::collections::HashMap;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use cm_core::{BrowseSort, Entry};
use uuid::Uuid;

use super::{
    HighlightStyle, SHORT_ID_LEN, SHORT_ID_LEN_EXTENDED, SNIPPET_MAX_BYTES, collapse_whitespace,
    compute_dedup_hints, detect_id_collisions, hoist_uniform, kind_histogram, relative_age,
    render_histogram, scope_histogram, short_id, smart_snippet,
};
use crate::browse::{BrowseRequest, BrowseResult};

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
    render_entries(&mut out, entries, now, &hoists, &result.relation_counts);
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

fn render_entries(
    out: &mut String,
    entries: &[Entry],
    now: DateTime<Utc>,
    hoists: &Hoists,
    relation_counts: &HashMap<Uuid, u32>,
) {
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

    // Intra-response dedup pass: first row carrying a given content
    // hash prefix is the leader; later rows with the same prefix pick
    // up a `dup_of: <leader short id>` annotation on their trailing
    // YAML comment. Computed once per response.
    let entry_refs: Vec<&Entry> = entries.iter().collect();
    let dedup = compute_dedup_hints(&entry_refs);

    for (entry, id_str) in entries.iter().zip(id_strings.iter()) {
        let sid = short_id(id_str, id_len);
        let _ = writeln!(out, "  - {sid}  {}", entry.title);

        let snippet = smart_snippet(&entry.body, None, HighlightStyle::None, SNIPPET_MAX_BYTES);
        let snippet_line = collapse_whitespace(&snippet);
        if !snippet_line.is_empty() {
            let _ = writeln!(out, "{cont_indent}{snippet_line}");
        }

        let dup_of = dedup
            .get(&entry.id)
            .map(|leader_uuid| short_id(&leader_uuid.to_string(), id_len).to_owned());
        let rels = relation_counts.get(&entry.id).copied().unwrap_or(0);
        let comment = render_row_comment(entry, now, hoists, dup_of.as_deref(), rels);
        let _ = writeln!(out, "{cont_indent}# {comment}");
    }
}

fn render_row_comment(
    entry: &Entry,
    now: DateTime<Utc>,
    hoists: &Hoists,
    dup_of: Option<&str>,
    rels: u32,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(6);
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
    if let Some(dup) = dup_of {
        parts.push(format!("dup_of: {dup}"));
    }
    if rels > 0 {
        parts.push(format!("rels: {rels}"));
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
///
/// Crate-visible so [`crate::projection::web_view`] can reuse the exact
/// same rendering for `WebBrowseHeader::sort_used` — the web view and the
/// YAML view must agree on the sort string, otherwise clients that
/// read both will see a mental-model drift.
pub(crate) fn sort_as_str(sort: BrowseSort) -> &'static str {
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
}
