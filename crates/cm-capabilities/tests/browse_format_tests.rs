//! Snapshot tests for `cm_capabilities::projection::format_browse_view`.
//!
//! Rebuilds Stuart's three-row session-log example from the research
//! doc §5.2.1 against a pinned `now`, renders via
//! [`format_browse_view_at`], and diffs byte-for-byte against the
//! golden file on disk. The golden locks down the wire shape across
//! future refactors; any intentional change must update the golden.
//!
//! No SQLite store is involved. The formatter is pure (`BrowseResult`
//! in, `String` out) so every fixture is built inline.

use chrono::{DateTime, Duration, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::browse::{BrowseRequest, BrowseResult};
use cm_capabilities::projection::format_browse_view_at;
use cm_core::{BrowseSort, Entry, EntryKind, EntryMeta, ScopePath};

/// The golden file for the canonical three-row session-log example.
const GOLDEN_SESSION_LOG: &str = include_str!("snapshots/browse_view_session_log.txt");

/// Pinned reference `now` for the snapshot. Every fixture timestamp is
/// expressed as `fixed_now() - Duration::...` so the rendered `age:`
/// column is deterministic.
fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

fn make_entry(
    id_hex: &str,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
) -> Entry {
    Entry {
        id: Uuid::parse_str(id_hex).expect("test fixture uuid parses"),
        scope_path: ScopePath::parse(scope).expect("test fixture scope parses"),
        kind: EntryKind::Observation,
        title: title.to_owned(),
        body: body.to_owned(),
        content_hash: "0".repeat(64),
        meta: Some(EntryMeta {
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            ..Default::default()
        }),
        created_by: "agent:claude-code".to_owned(),
        created_at: updated_at,
        updated_at,
        superseded_by: None,
    }
}

fn session_log_fixture() -> (BrowseResult, BrowseRequest, DateTime<Utc>) {
    let now = fixed_now();
    let entries = vec![
        make_entry(
            "019d79d3-0000-7000-8000-000000000001",
            "Session: marketing strategy + lazy tool loading design sketch",
            "## Task\n\nBrief session on marketing copy refinements for the launch. Validated lazy tool loading design sketch against three adapters.",
            "global",
            &["session-log", "marketing"],
            now - Duration::hours(2),
        ),
        make_entry(
            "019d6f22-0000-7000-8000-000000000002",
            "Session: cx_recall FTS operator regression",
            "Repro: queries with hyphens now fail with \"fts5: syntax error near '-'\". Likely introduced by the tokenizer change in ALP-1682.",
            "global/project:helioy",
            &["session-log", "cm"],
            now - Duration::hours(25),
        ),
        make_entry(
            "019d5bbb-0000-7000-8000-000000000003",
            "Session: worktree cleanup sweep",
            "Removed stale worktrees for ALP-1720 through ALP-1724 after the merge. Freed ~2.3 GB of disk.",
            "global",
            &["session-log"],
            now - Duration::days(6),
        ),
    ];

    let result = BrowseResult {
        entries,
        total: 113,
        next_cursor: Some("eyJzb3J0IjoicmVjZW50IiwibGFzdCI6ImZvbyJ9".to_owned()),
        has_more: true,
        sort_used: BrowseSort::Recent,
    };

    let request = BrowseRequest {
        tag: Some("session-log".to_owned()),
        limit: 50,
        ..Default::default()
    };

    (result, request, now)
}

#[test]
fn format_browse_view_matches_session_log_golden() {
    let (result, request, now) = session_log_fixture();
    let rendered = format_browse_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SESSION_LOG,
        "rendered browse view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_browse_view_session_log_stays_under_1200_bytes() {
    let (result, request, now) = session_log_fixture();
    let rendered = format_browse_view_at(&result, &request, now);
    assert!(
        rendered.len() <= 1200,
        "rendered browse view is {} bytes, exceeds 1200-byte budget:\n{rendered}",
        rendered.len(),
    );
}

#[test]
fn format_browse_view_empty_result_renders_clean() {
    let now = fixed_now();
    let result = BrowseResult {
        entries: Vec::new(),
        total: 0,
        next_cursor: None,
        has_more: false,
        sort_used: BrowseSort::Recent,
    };
    let request = BrowseRequest {
        limit: 50,
        ..Default::default()
    };

    let rendered = format_browse_view_at(&result, &request, now);
    // Header has no filters → `query:` line omitted.
    assert!(!rendered.contains("query:"), "empty result:\n{rendered}");
    assert!(rendered.contains("total: 0\n"), "empty result:\n{rendered}");
    assert!(
        rendered.contains("returned: 0\n"),
        "empty result:\n{rendered}"
    );
    // No entries → histograms and hoisted fields skipped.
    assert!(!rendered.contains("scope:"), "empty result:\n{rendered}");
    assert!(!rendered.contains("kinds:"), "empty result:\n{rendered}");
    assert!(
        !rendered.contains("created_by:"),
        "empty result:\n{rendered}"
    );
    // Empty entries list renders as YAML `[]`.
    assert!(
        rendered.contains("entries:\n  []\n"),
        "empty result:\n{rendered}"
    );
    // No pagination hint when !has_more.
    assert!(!rendered.contains("more -"), "empty result:\n{rendered}");
}

#[test]
fn format_browse_view_single_entry_hoists_all_uniform_fields() {
    let now = fixed_now();
    let entry = make_entry(
        "019d79d3-0000-7000-8000-000000000001",
        "Fact: the sky is blue",
        "Observed at 2026-04-11T11:45:00Z under clear conditions.",
        "global",
        &["weather"],
        now - Duration::minutes(15),
    );
    let result = BrowseResult {
        entries: vec![entry],
        total: 1,
        next_cursor: None,
        has_more: false,
        sort_used: BrowseSort::Recent,
    };
    let request = BrowseRequest {
        limit: 50,
        ..Default::default()
    };

    let rendered = format_browse_view_at(&result, &request, now);
    // All three uniform-hoistable fields surface in the header.
    assert!(rendered.contains("scope: global\n"), "\n{rendered}");
    assert!(rendered.contains("kinds: observation=1\n"), "\n{rendered}");
    assert!(
        rendered.contains("created_by: agent:claude-code  # uniform\n"),
        "\n{rendered}",
    );
    // The row comment drops `scope:` and `created_by:` (they are in the
    // header) and keeps `tags:` and `age:`.
    assert!(
        rendered.contains("# tags: weather  age: 15m"),
        "\n{rendered}",
    );
    assert!(
        !rendered.contains("scope: global  tags"),
        "row comment should omit hoisted scope\n{rendered}",
    );
}
