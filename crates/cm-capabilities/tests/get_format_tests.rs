//! Snapshot tests for `cm_capabilities::projection::format_get_view`.
//!
//! Four golden fixtures cover the render shapes that the formatter
//! materially varies between:
//!
//!   * `all_found` — every requested id resolves; header omits the
//!     `missing:` line and no trailer advisory is emitted.
//!   * `partial_missing` — two rows found, one missing; header carries
//!     the `missing:` list and the trailer advisory echoes it in
//!     natural-language form.
//!   * `all_missing` — zero rows found; entries block is omitted
//!     entirely, missing list and trailer still render.
//!   * `multiline_body` — a single entry whose body contains
//!     backticks, colons, `---`, unicode, and an embedded blank line.
//!     Exercises the YAML block-literal (`|`) rendering path and
//!     guards against character-class regressions that would force
//!     JSON-style escaping.
//!
//! Each test renders via [`format_get_view_at`] with a pinned `now`
//! and diffs byte-for-byte against the golden on disk. Any intentional
//! wire-shape change must update the golden.
//!
//! Plus four behavioural assertions on top of the byte-for-byte
//! checks: missing-line omission on all-found, advisory emission on
//! partial-missing, entries-block omission on all-missing, and a
//! cross-fixture guard that `content_hash` never leaks into the
//! rendered output (parent ticket locked decision 8).

use chrono::{DateTime, Duration, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::projection::format_get_view_at;
use cm_core::{Confidence, Entry, EntryKind, EntryMeta, ScopePath};

const GOLDEN_ALL_FOUND: &str = include_str!("snapshots/get_view_all_found.txt");
const GOLDEN_PARTIAL_MISSING: &str = include_str!("snapshots/get_view_partial_missing.txt");
const GOLDEN_ALL_MISSING: &str = include_str!("snapshots/get_view_all_missing.txt");
const GOLDEN_MULTILINE_BODY: &str = include_str!("snapshots/get_view_multiline_body.txt");

fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn make_entry(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    confidence: Option<Confidence>,
    updated_at: DateTime<Utc>,
) -> Entry {
    let meta = if tags.is_empty() && confidence.is_none() {
        None
    } else {
        Some(EntryMeta {
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            confidence,
            ..Default::default()
        })
    };
    Entry {
        id: Uuid::parse_str(id_hex).expect("test fixture uuid parses"),
        scope_path: ScopePath::parse(scope).expect("test fixture scope parses"),
        kind,
        title: title.to_owned(),
        body: body.to_owned(),
        content_hash: "0".repeat(64),
        meta,
        created_by: "agent:claude-code".to_owned(),
        created_at: updated_at,
        updated_at,
        superseded_by: None,
    }
}

/// `all_found` fixture: two entries, both resolve. One entry carries
/// full metadata (tags + confidence), the other has `meta: None` to
/// exercise the conditional tag/confidence rendering.
fn all_found_fixture() -> (Vec<Entry>, Vec<String>, DateTime<Utc>) {
    let now = fixed_now();
    let entries = vec![
        make_entry(
            "019d8a01-0000-7000-8000-000000000001",
            EntryKind::Decision,
            "Snippet truncation at word boundaries",
            "The byte-prefix snippet drops mid-word; floor_char_boundary\n\
             plus a word-boundary walk gives a snippet strategy that\n\
             keeps tokens whole.",
            "global/project:helioy",
            &["projection", "snippet"],
            Some(Confidence::High),
            now - Duration::hours(25),
        ),
        make_entry(
            "019d8a01-0000-7000-8000-000000000002",
            EntryKind::Lesson,
            "Always round down to char boundaries",
            "Str indexing at a byte offset that lands inside a multi-byte\n\
             character panics at runtime.",
            "global",
            &[],
            None,
            now - Duration::hours(3),
        ),
    ];
    let requested = vec![
        "019d8a01-0000-7000-8000-000000000001".to_owned(),
        "019d8a01-0000-7000-8000-000000000002".to_owned(),
    ];
    (entries, requested, now)
}

/// `partial_missing` fixture: three ids requested, two resolve. The
/// missing id is third in the request order so the test also verifies
/// the missing-list preserves the caller's order.
fn partial_missing_fixture() -> (Vec<Entry>, Vec<String>, DateTime<Utc>) {
    let now = fixed_now();
    let entries = vec![
        make_entry(
            "019d8a01-0000-7000-8000-000000000001",
            EntryKind::Fact,
            "Found entry one",
            "Body for found entry one.",
            "global",
            &[],
            None,
            now - Duration::hours(2),
        ),
        make_entry(
            "019d8a01-0000-7000-8000-000000000002",
            EntryKind::Fact,
            "Found entry two",
            "Body for found entry two.",
            "global",
            &[],
            None,
            now - Duration::hours(5),
        ),
    ];
    let requested = vec![
        "019d8a01-0000-7000-8000-000000000001".to_owned(),
        "019d8a01-0000-7000-8000-000000000002".to_owned(),
        "019d8a01-0000-7000-8000-00000000ffff".to_owned(),
    ];
    (entries, requested, now)
}

/// `all_missing` fixture: zero entries resolve, two requested. The
/// entries block is suppressed entirely and only the header + trailer
/// advisory render.
fn all_missing_fixture() -> (Vec<Entry>, Vec<String>, DateTime<Utc>) {
    let now = fixed_now();
    let entries: Vec<Entry> = Vec::new();
    let requested = vec![
        "019d8a01-0000-7000-8000-00000000ffff".to_owned(),
        "019d8a01-0000-7000-8000-00000000fffe".to_owned(),
    ];
    (entries, requested, now)
}

/// `multiline_body` fixture: one entry, multi-line body with a variety
/// of YAML-hostile characters and an embedded blank line. Verifies
/// the block-literal renderer passes backticks, colons, `---`, unicode
/// (CJK + emoji), and blank lines through without any escaping.
fn multiline_body_fixture() -> (Vec<Entry>, Vec<String>, DateTime<Utc>) {
    let now = fixed_now();
    let entries = vec![make_entry(
        "019d8a01-0000-7000-8000-000000000001",
        EntryKind::Observation,
        "Multiline body with special chars",
        "Line one with `backticks` and : colons.\n\
         Line two with --- yaml separator.\n\
         Line three with unicode: 日本語 and emoji 🎯.\n\
         \n\
         Line five after blank line.",
        "global",
        &["edge-case", "yaml"],
        None,
        now - Duration::hours(25),
    )];
    let requested = vec!["019d8a01-0000-7000-8000-000000000001".to_owned()];
    (entries, requested, now)
}

#[test]
fn format_get_view_all_found_matches_golden() {
    let (entries, requested, now) = all_found_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert_eq!(
        rendered, GOLDEN_ALL_FOUND,
        "all_found rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_get_view_partial_missing_matches_golden() {
    let (entries, requested, now) = partial_missing_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert_eq!(
        rendered, GOLDEN_PARTIAL_MISSING,
        "partial_missing rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_get_view_all_missing_matches_golden() {
    let (entries, requested, now) = all_missing_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert_eq!(
        rendered, GOLDEN_ALL_MISSING,
        "all_missing rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_get_view_multiline_body_matches_golden() {
    let (entries, requested, now) = multiline_body_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert_eq!(
        rendered, GOLDEN_MULTILINE_BODY,
        "multiline_body rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_get_view_all_found_omits_missing_line_and_trailer() {
    let (entries, requested, now) = all_found_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert!(
        !rendered.contains("missing:"),
        "all_found rendering should not contain a missing: header line:\n{rendered}",
    );
    assert!(
        !rendered.contains(" missing - ids:"),
        "all_found rendering should not contain a missing trailer advisory:\n{rendered}",
    );
    assert!(
        rendered.contains("entries:\n"),
        "all_found rendering must still contain the entries block:\n{rendered}",
    );
}

#[test]
fn format_get_view_partial_missing_emits_list_and_advisory() {
    let (entries, requested, now) = partial_missing_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert!(
        rendered.contains("missing: [019d8a01-0000-7000-8000-00000000ffff]"),
        "partial_missing rendering should carry the explicit missing list:\n{rendered}",
    );
    assert!(
        rendered.contains("# 1 missing - ids: 019d8a01-0000-7000-8000-00000000ffff"),
        "partial_missing rendering should carry the trailer advisory:\n{rendered}",
    );
    assert!(
        rendered.contains("entries:\n"),
        "partial_missing rendering must still carry the entries block:\n{rendered}",
    );
}

#[test]
fn format_get_view_all_missing_omits_entries_block() {
    let (entries, requested, now) = all_missing_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert!(
        !rendered.contains("entries:"),
        "all_missing rendering must not emit an entries: key:\n{rendered}",
    );
    assert!(
        rendered.contains("found: 0"),
        "all_missing rendering must show found: 0:\n{rendered}",
    );
    assert!(
        rendered.contains("# 2 missing - ids: "),
        "all_missing rendering must carry the trailer advisory:\n{rendered}",
    );
}

#[test]
fn format_get_view_multiline_body_preserves_special_chars_in_block_literal() {
    let (entries, requested, now) = multiline_body_fixture();
    let rendered = format_get_view_at(&entries, &requested, now);
    assert!(
        rendered.contains("body: |\n"),
        "multiline_body rendering must open a block literal:\n{rendered}",
    );
    for expected in [
        "`backticks`",
        ": colons",
        "--- yaml separator",
        "日本語",
        "🎯",
        "Line five after blank line.",
    ] {
        assert!(
            rendered.contains(expected),
            "multiline_body rendering must preserve {expected:?}:\n{rendered}",
        );
    }
}

#[test]
fn format_get_view_never_emits_content_hash() {
    // Parent ticket locked decision 8: content_hash is 64 hex chars
    // of debug-only data and must never appear in the cx_get response.
    // Every fixture is checked (including the one with fully-populated
    // metadata) to guard against a regression that conditionally emits
    // it on the meta-populated path.
    let placeholder_hash = "0".repeat(64);
    for (entries, requested, now) in [
        all_found_fixture(),
        partial_missing_fixture(),
        all_missing_fixture(),
        multiline_body_fixture(),
    ] {
        let rendered = format_get_view_at(&entries, &requested, now);
        assert!(
            !rendered.contains("content_hash"),
            "content_hash key leaked into rendering:\n{rendered}",
        );
        assert!(
            !rendered.contains(&placeholder_hash),
            "content_hash hex leaked into rendering:\n{rendered}",
        );
    }
}
