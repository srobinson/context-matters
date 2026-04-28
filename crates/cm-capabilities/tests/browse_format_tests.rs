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

use std::collections::HashMap;

use chrono::{DateTime, Duration, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::browse::{BrowseRequest, BrowseResult};
use cm_capabilities::projection::format_browse_view_at;
use cm_capabilities::scope::{
    BrowseScopeMode, CWD_INFERRED_SCOPE, ScopeResolution, ScopeResolutionCandidate,
    ScopeResolutionConfidence, ScopeSelector,
};
use cm_core::{BrowseSort, Entry, EntryKind, EntryMeta, ScopePath};

/// The golden file for the canonical three-row session-log example.
const GOLDEN_SESSION_LOG: &str = include_str!("snapshots/browse_view_session_log.txt");

/// Pinned reference `now` for the snapshot. Every fixture timestamp is
/// expressed as `fixed_now() - Duration::...` so the rendered `age:`
/// column is deterministic.
fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

/// Derives a unique 64-char hex `content_hash` from the test row's
/// `id_hex` so every fixture row hashes differently by default. Keeps
/// the intra-response dedup pass from flagging unrelated test rows as
/// dupes just because they all share a placeholder hash.
fn content_hash_from(id_hex: &str) -> String {
    let clean = id_hex.replace('-', "");
    assert!(
        clean.len() <= 64,
        "test fixture id_hex must fit inside 64 hex chars",
    );
    format!("{clean:0<64}")
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
        content_hash: content_hash_from(id_hex),
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
        scope_used: None,
        include_resolution: false,
        limit_used: 50,
        sort_used: BrowseSort::Recent,
        relation_counts: HashMap::new(),
        resolution: None,
        advisory: None,
    };

    let request = BrowseRequest {
        tag: Some("session-log".to_owned()),
        limit: Some(50),
        ..Default::default()
    };

    (result, request, now)
}

fn smart_scope_resolution_fixture() -> ScopeResolution {
    ScopeResolution {
        requested_scope: CWD_INFERRED_SCOPE.to_owned(),
        resolved_scope: ScopePath::parse("global/project:helioy/repo:context-matters")
            .expect("test fixture scope parses"),
        scope_mode: BrowseScopeMode::Resolved,
        confidence: ScopeResolutionConfidence::High,
        candidates: vec![
            ScopeResolutionCandidate {
                scope: ScopePath::parse("global/project:helioy/repo:context-matters")
                    .expect("test fixture scope parses"),
                score: 330,
                matched: vec![
                    "repo".to_owned(),
                    "project_parent".to_owned(),
                    "specificity".to_owned(),
                ],
            },
            ScopeResolutionCandidate {
                scope: ScopePath::parse("global/project:helioy")
                    .expect("test fixture scope parses"),
                score: 110,
                matched: vec!["project_parent".to_owned(), "project".to_owned()],
            },
        ],
        signals: vec![
            "cwd basename matched repo scope segment: context-matters".to_owned(),
            "cwd parent basename matched project scope segment: helioy".to_owned(),
        ],
    }
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
fn format_browse_view_omits_resolution_for_legacy_fixture() {
    let (result, request, now) = session_log_fixture();
    let rendered = format_browse_view_at(&result, &request, now);

    assert!(
        !rendered.contains("resolution:"),
        "legacy browse output should not grow resolution metadata:\n{rendered}",
    );
}

#[test]
fn format_browse_view_renders_cwd_inferred_scope_resolution() {
    let (mut result, mut request, now) = session_log_fixture();
    result.resolution = Some(smart_scope_resolution_fixture());
    result.scope_used = Some(CWD_INFERRED_SCOPE.to_owned());
    result.include_resolution = true;
    request.scope = Some(ScopeSelector::cwd_inferred(None));
    request.include_resolution = Some(true);

    let rendered = format_browse_view_at(&result, &request, now);

    assert!(
        rendered.contains("query: scope=cwd_inferred tag=session-log\n"),
        "cwd_inferred scope should be visible in the query header:\n{rendered}",
    );
    assert!(
        rendered.contains("requested_scope: cwd_inferred\n"),
        "requested scope missing from YAML resolution block:\n{rendered}",
    );
    assert!(
        rendered.contains("resolved_scope: global/project:helioy/repo:context-matters\n"),
        "resolved scope missing from YAML resolution block:\n{rendered}",
    );
    assert!(
        rendered.contains("scope_mode: resolved\n"),
        "scope mode missing from YAML resolution block:\n{rendered}",
    );
    assert!(
        rendered.contains("confidence: high\n"),
        "confidence missing from YAML resolution block:\n{rendered}",
    );
    assert!(
        rendered.contains("    - \"cwd basename matched repo scope segment: context-matters\"\n"),
        "repo signal missing from YAML resolution block:\n{rendered}",
    );
    assert!(
        rendered.contains("    - \"cwd parent basename matched project scope segment: helioy\"\n"),
        "project signal missing from YAML resolution block:\n{rendered}",
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
        scope_used: None,
        include_resolution: false,
        limit_used: 50,
        sort_used: BrowseSort::Recent,
        relation_counts: HashMap::new(),
        resolution: None,
        advisory: None,
    };
    let request = BrowseRequest {
        limit: Some(50),
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
fn format_browse_view_rels_annotation_fires_only_for_populated_rows() {
    // Relation-count annotations: entry 1 has 2 outgoing edges
    // declared in `relation_counts`, entries 2 and 3 are absent from
    // the map. The renderer must emit `rels: 2` on entry 1's trailing
    // comment and leave the other two rows untouched. Asserted
    // behaviourally rather than via a golden snapshot so any future
    // row-comment reshuffle can be validated with a targeted rerun.
    let (mut result, request, now) = session_log_fixture();
    let target_id = result.entries[0].id;
    let mut counts: HashMap<Uuid, u32> = HashMap::new();
    counts.insert(target_id, 2);
    result.relation_counts = counts;

    let rendered = format_browse_view_at(&result, &request, now);
    assert!(
        rendered.contains("rels: 2"),
        "entry 1 should carry rels: 2:\n{rendered}",
    );
    assert_eq!(
        rendered.matches("rels: ").count(),
        1,
        "exactly one rels annotation expected (entry 1 only):\n{rendered}",
    );
    // Row 1's comment carries the annotation at the end of the line;
    // rows 2 and 3 must not mention `rels:` anywhere in their comments.
    let comment_lines: Vec<&str> = rendered
        .lines()
        .filter(|l| l.trim_start().starts_with("# "))
        .collect();
    assert!(
        comment_lines
            .iter()
            .any(|l| l.contains("age: 2h") && l.contains("rels: 2")),
        "entry 1's comment should carry rels: 2:\n{rendered}",
    );
    assert!(
        comment_lines
            .iter()
            .filter(|l| !l.contains("age: 2h"))
            .all(|l| !l.contains("rels:")),
        "only entry 1 should carry rels:\n{rendered}",
    );
}

#[test]
fn format_browse_view_drill_down_advisory_fires_on_dominant_kind() {
    // Faceted drill-down advisory: the session-log fixture carries
    // 3/3 `observation` rows (100%), well above the 60% dominance
    // threshold, so the trailer must append a `# narrow: cx_browse(...)`
    // line keyed on the dominant kind. Browse uses singular filter
    // args (`kind=observation`, no brackets) where recall would emit
    // the plural array form, so the rendered call shape differs from
    // the recall-side advisory by design.
    let (result, request, now) = session_log_fixture();
    let rendered = format_browse_view_at(&result, &request, now);
    let expected = "# narrow: cx_browse(kind=observation) - 3 of 3 results are observation";
    assert!(
        rendered.contains(expected),
        "drill-down advisory line missing or malformed:\n{rendered}",
    );
    // The advisory must fire exactly once per response.
    assert_eq!(
        rendered.matches("# narrow:").count(),
        1,
        "exactly one drill-down advisory expected:\n{rendered}",
    );
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
        scope_used: None,
        include_resolution: false,
        limit_used: 50,
        sort_used: BrowseSort::Recent,
        relation_counts: HashMap::new(),
        resolution: None,
        advisory: None,
    };
    let request = BrowseRequest {
        limit: Some(50),
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
