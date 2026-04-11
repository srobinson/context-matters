//! `cx_get` YAML-text formatter for MCP response bodies.
//!
//! Consumed by `cx_get` (via the wire-swap sub that lands the YAML
//! envelope) to replace the double-encoded JSON-in-text response shape
//! with a compact, agent-legible YAML detail view. The target shape is
//! described in `research/cx-response-payload-redesign-context-matters.md`
//! §5.2.3.
//!
//! Unlike browse/recall the view renders *full* entries: full UUID in
//! the `id:` field, full body in a YAML block literal, full metadata
//! (except `content_hash`, which is unconditionally hidden per locked
//! decision 8 in the parent ticket — it is 64 hex chars of
//! debug-only data). The key deliverable is explicit enumeration of
//! requested ids that the store did not return, so the caller sees
//! exactly which lookups failed rather than inferring from a count.
//!
//! The formatter is pure text: no I/O, no allocations beyond the
//! output string and its temporaries. The only non-deterministic
//! input is the reference `now` used for relative-age rendering,
//! which is captured once at the entry point and injected into
//! [`format_get_view_at`] so snapshot tests can pin the `age:` column.

use std::collections::HashSet;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use cm_core::{Confidence, Entry};

use super::relative_age;

/// Maximum number of missing ids listed explicitly in the trailing
/// advisory before appending `, ...`. Keeps the trailer bounded on
/// large request sets; the full list is still carried in the
/// structured `missing:` header field, so no information is lost.
const TRAILER_MAX_IDS: usize = 6;

/// Render a [`cx_get`](crate) response body given the store-returned
/// entries and the raw list of requested id strings. The missing-id
/// diff (`requested \ found.id`) is computed inside the formatter, so
/// the caller does not need to pre-compute it.
///
/// Captures `Utc::now()` once for relative-age formatting and delegates
/// to [`format_get_view_at`]. Use the `_at` variant from tests that
/// need the rendered `age:` column to be deterministic.
pub fn format_get_view(found: &[Entry], requested: &[String]) -> String {
    format_get_view_at(found, requested, Utc::now())
}

/// Deterministic variant of [`format_get_view`] that takes an explicit
/// reference `now` for relative-age rendering. Production callers should
/// prefer [`format_get_view`]; this entry point exists so snapshot
/// tests can pin the `age:` column without touching the system clock.
pub fn format_get_view_at(found: &[Entry], requested: &[String], now: DateTime<Utc>) -> String {
    let missing = compute_missing(found, requested);
    let mut out = String::with_capacity(1024);
    out.push_str("---\n");
    render_header(&mut out, found, requested, &missing);
    if !found.is_empty() {
        out.push('\n');
        out.push_str("entries:\n");
        for entry in found {
            render_entry(&mut out, entry, now);
        }
    }
    if !missing.is_empty() {
        out.push('\n');
        render_missing_trailer(&mut out, &missing);
    }
    out
}

/// Compute requested ids that the store did not return. Preserves the
/// caller's original string form rather than the store-canonical one,
/// so the rendered `missing:` list exactly echoes what the caller asked
/// for. Direct string comparison against the canonical uuid form is
/// safe because `cx_get` already parses every requested string through
/// `uuid::Uuid::parse_str` before calling the store — by the time the
/// strings reach this formatter they are already in canonical form.
fn compute_missing<'a>(found: &[Entry], requested: &'a [String]) -> Vec<&'a str> {
    let found_ids: HashSet<String> = found.iter().map(|e| e.id.to_string()).collect();
    requested
        .iter()
        .filter(|req| !found_ids.contains(*req))
        .map(|s| s.as_str())
        .collect()
}

fn render_header(out: &mut String, found: &[Entry], requested: &[String], missing: &[&str]) {
    let _ = writeln!(out, "requested: {}", requested.len());
    let _ = writeln!(out, "found: {}", found.len());
    if !missing.is_empty() {
        let rendered = missing.join(", ");
        let _ = writeln!(out, "missing: [{rendered}]");
    }
}

fn render_entry(out: &mut String, entry: &Entry, now: DateTime<Utc>) {
    let _ = writeln!(out, "  - id: {}", entry.id);
    let _ = writeln!(out, "    title: {}", entry.title);
    let _ = writeln!(out, "    scope: {}", entry.scope_path.as_str());
    let _ = writeln!(out, "    kind: {}", entry.kind.as_str());

    if let Some(meta) = entry.meta.as_ref() {
        if !meta.tags.is_empty() {
            let rendered = meta.tags.join(", ");
            let _ = writeln!(out, "    tags: [{rendered}]");
        }
        if let Some(conf) = meta.confidence {
            let _ = writeln!(out, "    confidence: {}", confidence_as_str(conf));
        }
    }

    let _ = writeln!(out, "    age: {}", relative_age(entry.updated_at, now));
    render_body(out, &entry.body);
}

/// Render `entry.body` under a `body:` key, using a YAML block literal
/// (`|`) for non-empty bodies so backticks, colons, unicode, and
/// embedded `---` separators pass through verbatim without any JSON
/// escaping. An empty body falls back to a double-quoted empty scalar
/// so the resulting YAML still type-checks.
///
/// Line-ending handling:
///
/// * Exactly one trailing newline in the input is stripped before
///   emitting, so the block literal does not render a dangling blank
///   line after the final content line.
/// * Embedded blank lines inside the body are preserved as bare `\n`
///   (no indent). YAML 1.2 §8.1.1.1 treats empty lines inside a block
///   scalar as line-folding, not content, so this is both semantically
///   correct and avoids trailing-whitespace noise in the rendered
///   output.
fn render_body(out: &mut String, body: &str) {
    if body.is_empty() {
        let _ = writeln!(out, "    body: \"\"");
        return;
    }
    out.push_str("    body: |\n");
    let trimmed = body.strip_suffix('\n').unwrap_or(body);
    for line in trimmed.split('\n') {
        if line.is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "      {line}");
        }
    }
}

/// Render the `# N missing - ids: ...` trailing advisory. Truncates
/// the id list at [`TRAILER_MAX_IDS`] with a `, ...` sentinel so the
/// trailer stays bounded when the caller requested a large batch; the
/// structured `missing:` header field still carries the full list.
fn render_missing_trailer(out: &mut String, missing: &[&str]) {
    let count = missing.len();
    let (shown, truncated) = if missing.len() > TRAILER_MAX_IDS {
        (&missing[..TRAILER_MAX_IDS], true)
    } else {
        (missing, false)
    };
    let ids = shown.join(", ");
    let tail = if truncated { ", ..." } else { "" };
    let _ = writeln!(out, "# {count} missing - ids: {ids}{tail}");
}

/// Lowercase snake_case name for a [`Confidence`] variant. Matches the
/// `serde(rename_all = "snake_case")` rendering on the enum so log
/// channels and the text envelope agree on the canonical name. The
/// enum has no `Display`/`as_str`, so this inline helper is the single
/// source of truth for the text form. `pub(crate)` so the sibling
/// `web_view::project_web_get` projection can use the same mapping
/// without duplicating the match arms (DRY rule from CLAUDE.md).
pub(crate) fn confidence_as_str(c: Confidence) -> &'static str {
    match c {
        Confidence::High => "high",
        Confidence::Medium => "medium",
        Confidence::Low => "low",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use cm_core::{EntryKind, ScopePath};
    use uuid::Uuid;

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
    }

    fn minimal_entry(id_hex: &str) -> Entry {
        Entry {
            id: Uuid::parse_str(id_hex).unwrap(),
            scope_path: ScopePath::parse("global").unwrap(),
            kind: EntryKind::Fact,
            title: "t".to_owned(),
            body: String::new(),
            content_hash: "0".repeat(64),
            meta: None,
            created_by: "test".to_owned(),
            created_at: fixed_now(),
            updated_at: fixed_now(),
            superseded_by: None,
        }
    }

    #[test]
    fn compute_missing_returns_requested_not_in_found_preserving_order() {
        let entries = vec![
            minimal_entry("019d8a01-0000-7000-8000-000000000001"),
            minimal_entry("019d8a01-0000-7000-8000-000000000002"),
        ];
        let requested = vec![
            "019d8a01-0000-7000-8000-000000000001".to_owned(),
            "019d8a01-0000-7000-8000-000000000003".to_owned(),
            "019d8a01-0000-7000-8000-000000000002".to_owned(),
            "019d8a01-0000-7000-8000-00000000ffff".to_owned(),
        ];
        let missing = compute_missing(&entries, &requested);
        assert_eq!(
            missing,
            vec![
                "019d8a01-0000-7000-8000-000000000003",
                "019d8a01-0000-7000-8000-00000000ffff"
            ]
        );
    }

    #[test]
    fn compute_missing_all_found_returns_empty() {
        let entries = vec![minimal_entry("019d8a01-0000-7000-8000-000000000001")];
        let requested = vec!["019d8a01-0000-7000-8000-000000000001".to_owned()];
        assert!(compute_missing(&entries, &requested).is_empty());
    }

    #[test]
    fn compute_missing_all_missing_returns_every_requested() {
        let entries: Vec<Entry> = Vec::new();
        let requested = vec![
            "019d8a01-0000-7000-8000-000000000001".to_owned(),
            "019d8a01-0000-7000-8000-000000000002".to_owned(),
        ];
        assert_eq!(compute_missing(&entries, &requested).len(), 2);
    }

    #[test]
    fn confidence_as_str_covers_every_variant() {
        assert_eq!(confidence_as_str(Confidence::High), "high");
        assert_eq!(confidence_as_str(Confidence::Medium), "medium");
        assert_eq!(confidence_as_str(Confidence::Low), "low");
    }

    #[test]
    fn render_body_empty_uses_quoted_literal() {
        let mut out = String::new();
        render_body(&mut out, "");
        assert_eq!(out, "    body: \"\"\n");
    }

    #[test]
    fn render_body_strips_exactly_one_trailing_newline() {
        let mut out = String::new();
        render_body(&mut out, "line one\n");
        assert_eq!(out, "    body: |\n      line one\n");
    }

    #[test]
    fn render_body_preserves_embedded_blank_lines() {
        let mut out = String::new();
        render_body(&mut out, "line one\n\nline three");
        // Each non-empty line is indented 6 spaces; the embedded blank
        // line becomes a bare `\n` (no trailing whitespace).
        assert_eq!(out, "    body: |\n      line one\n\n      line three\n",);
    }

    #[test]
    fn render_body_preserves_backticks_colons_and_unicode() {
        let mut out = String::new();
        render_body(&mut out, "Line `backticks`: with colon. Unicode: 日本語");
        assert!(out.contains("      Line `backticks`: with colon. Unicode: 日本語\n"));
    }

    #[test]
    fn render_missing_trailer_truncates_large_lists() {
        let mut out = String::new();
        let ids: Vec<String> = (0..10).map(|i| format!("id{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        render_missing_trailer(&mut out, &refs);
        assert!(out.starts_with("# 10 missing - ids: id0, id1, id2, id3, id4, id5, ...\n"));
    }

    #[test]
    fn render_missing_trailer_at_exact_max_does_not_truncate() {
        let mut out = String::new();
        let ids: Vec<String> = (0..TRAILER_MAX_IDS).map(|i| format!("id{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        render_missing_trailer(&mut out, &refs);
        assert!(!out.contains(", ..."));
    }
}
