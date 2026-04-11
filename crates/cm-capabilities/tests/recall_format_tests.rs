//! Snapshot tests for `cm_capabilities::projection::format_recall_view`.
//!
//! Builds three `RecallResult` fixtures covering the routing branches
//! that materially change the rendered shape:
//!
//!   * `Search` with populated BM25 scores — exercises the score column
//!     and the FTS5-routing advisory.
//!   * `BrowseFallback` without scores — exercises the no-score row
//!     shape and the browse-fallback advisory.
//!   * Empty result (any routing) — exercises the `no matches` trailer
//!     and verifies the header still renders.
//!
//! Each test renders via [`format_recall_view_at`] with a pinned `now`
//! and diffs byte-for-byte against the golden on disk. Any intentional
//! wire-shape change must update the golden.
//!
//! No SQLite store is involved. The formatter is pure (`RecallResult`
//! in, `String` out) so every fixture is built inline.

use chrono::{DateTime, Duration, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::projection::{RecallRow, format_recall_view_at};
use cm_capabilities::recall::{RecallRequest, RecallResult, RecallRouting, SearchTier};
use cm_core::{Entry, EntryKind, EntryMeta, ScopePath};

const GOLDEN_SEARCH: &str = include_str!("snapshots/recall_view_search.txt");
const GOLDEN_SEARCH_PREFIX_TIER: &str =
    include_str!("snapshots/recall_view_search_prefix_tier.txt");
const GOLDEN_SEARCH_SPLIT_OR_TIER: &str =
    include_str!("snapshots/recall_view_search_split_or_tier.txt");
const GOLDEN_BROWSE_FALLBACK: &str = include_str!("snapshots/recall_view_browse_fallback.txt");
const GOLDEN_EMPTY: &str = include_str!("snapshots/recall_view_empty.txt");
const GOLDEN_DEDUP: &str = include_str!("snapshots/recall_view_dedup.txt");

fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

/// Derives a unique 64-char hex `content_hash` from the test row's
/// `id_hex` so every fixture row hashes differently by default. Keeps
/// the intra-response dedup pass from flagging unrelated test rows as
/// dupes just because they all share a placeholder hash. Tests that
/// intentionally exercise the dedup codepath override this by calling
/// [`make_row_with_hash`] directly.
fn content_hash_from(id_hex: &str) -> String {
    let clean = id_hex.replace('-', "");
    assert!(
        clean.len() <= 64,
        "test fixture id_hex must fit inside 64 hex chars",
    );
    format!("{clean:0<64}")
}

#[allow(clippy::too_many_arguments)]
fn make_row(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
    score: Option<f32>,
) -> RecallRow {
    make_row_with_hash(
        id_hex,
        kind,
        title,
        body,
        scope,
        tags,
        updated_at,
        score,
        &content_hash_from(id_hex),
    )
}

#[allow(clippy::too_many_arguments)]
fn make_row_with_hash(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
    score: Option<f32>,
    content_hash: &str,
) -> RecallRow {
    RecallRow {
        entry: Entry {
            id: Uuid::parse_str(id_hex).expect("test fixture uuid parses"),
            scope_path: ScopePath::parse(scope).expect("test fixture scope parses"),
            kind,
            title: title.to_owned(),
            body: body.to_owned(),
            content_hash: content_hash.to_owned(),
            meta: Some(EntryMeta {
                tags: tags.iter().map(|t| (*t).to_owned()).collect(),
                ..Default::default()
            }),
            created_by: "agent:claude-code".to_owned(),
            created_at: updated_at,
            updated_at,
            superseded_by: None,
        },
        score,
    }
}

/// `Search` routing fixture: three rows, all carry a raw BM25 score,
/// mixed across kinds and scopes so the header histograms exercise
/// both code paths. The raw scores (-3.47, -1.12, -0.88) match the
/// values used in the normalise_bm25 unit test, so the formatter
/// output's score column directly reflects the test-documented
/// normalisation math (1.00, 0.09, 0.00).
fn search_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let now = fixed_now();
    let entries = vec![
        make_row(
            "019d8a01-0000-7000-8000-000000000001",
            EntryKind::Decision,
            "Snippet strategy: centre on first query-term match",
            "The byte-prefix snippet drops mid-word; floor_char_boundary \
             plus a word-boundary walk gives a snippet strategy that \
             keeps tokens whole without ever panicking on multi-byte UTF-8.",
            "global/project:helioy",
            &["projection", "snippet"],
            now - Duration::hours(25),
            Some(-3.47),
        ),
        make_row(
            "019d7f3e-0000-7000-8000-000000000002",
            EntryKind::Decision,
            "Query-centred snippet window has to survive empty queries",
            "When the caller passes an empty query string the smart_snippet \
             helper must fall back to the stripped body start instead of \
             centring on byte offset zero of a non-match.",
            "global/project:helioy",
            &["projection", "snippet", "edge-case"],
            now - Duration::hours(3),
            Some(-1.12),
        ),
        make_row(
            "019d6a22-0000-7000-8000-000000000003",
            EntryKind::Lesson,
            "Snippet truncation must respect UTF-8 char boundaries",
            "We learned the hard way: str indexing at a byte offset that \
             lands inside a multi-byte character panics at runtime. \
             Always round down to the nearest char boundary before slicing.",
            "global",
            &["projection"],
            now - Duration::days(5),
            Some(-0.88),
        ),
    ];

    let result = RecallResult {
        entries,
        scope_chain: vec!["global/project:helioy".to_owned(), "global".to_owned()],
        scope_hits: vec![
            ("global/project:helioy".to_owned(), 2),
            ("global".to_owned(), 1),
        ],
        token_estimate: 3_420,
        routing: RecallRouting::Search,
        tier: Some(SearchTier::Exact),
        candidates_before_filter: 47,
        fetch_limit_used: 50,
    };

    let request = RecallRequest {
        query: Some("snippet strategy".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };

    (result, request, now)
}

/// Cascade-tier override on top of [`search_fixture`]. Used by the
/// prefix and split_or tier snapshot tests so the three tier
/// renderings stay byte-identical except for the header suffix and
/// trailing advisory. Mutating the tier in place keeps the row data
/// and scores in one place (DRY) and means any future change to
/// `search_fixture` automatically propagates to every tier test.
fn search_fixture_with_tier(tier: SearchTier) -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let (mut result, request, now) = search_fixture();
    result.tier = Some(tier);
    (result, request, now)
}

/// `BrowseFallback` routing fixture: two rows, `score` is `None` on
/// every row (no FTS rank was computed), so the formatter must skip
/// the score column entirely. No query was supplied. The trailer
/// uses the browse-fallback advisory.
fn browse_fallback_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let now = fixed_now();
    let entries = vec![
        make_row(
            "019d8a01-0000-7000-8000-00000000000a",
            EntryKind::Fact,
            "Recent observation: build latency regressed after rustc bump",
            "Nightly CI went from 38s cold to 52s cold after the rustc bump. \
             Rolled back; watching for a stable release that restores parity.",
            "global",
            &["ci", "rustc"],
            now - Duration::hours(2),
            None,
        ),
        make_row(
            "019d7f3e-0000-7000-8000-00000000000b",
            EntryKind::Fact,
            "FTS MATCH with single apostrophe escapes are still broken",
            "Queries like `it's` trigger fts5: syntax error. Quoting with \
             double-quotes around the term is the workaround until the \
             tokenizer fix lands.",
            "global",
            &["fts", "bug"],
            now - Duration::hours(26),
            None,
        ),
    ];

    let result = RecallResult {
        entries,
        scope_chain: vec!["global".to_owned()],
        scope_hits: vec![("global".to_owned(), 2)],
        token_estimate: 220,
        routing: RecallRouting::BrowseFallback,
        tier: None,
        candidates_before_filter: 5,
        fetch_limit_used: 50,
    };

    let request = RecallRequest {
        query: None,
        limit: 50,
        max_tokens: None,
        ..Default::default()
    };

    (result, request, now)
}

/// Dedup fixture: three rows where rows 1 and 3 carry the same
/// BLAKE3-hash 16-char prefix (simulating "I stored the same lesson
/// twice") while row 2 is genuinely distinct. The expected rendering
/// annotates row 3 with `dup_of: <row 1 short id>` on its trailing
/// YAML comment and leaves the other two rows untouched.
fn dedup_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let now = fixed_now();
    // Two 16-char hex prefixes padded out to a full 64-char hash. The
    // dedup pass keys on the first 16 chars, so two rows that share a
    // leading `deaddeaddeaddead` will collide and the third row with
    // `cafecafecafecafe` will not.
    let hash_shared: String = format!("{:0<64}", "deaddeaddeaddead");
    let hash_unique: String = format!("{:0<64}", "cafecafecafecafe");
    let entries = vec![
        make_row_with_hash(
            "019dedaa-0000-7000-8000-000000000001",
            EntryKind::Lesson,
            "Stored the same lesson twice",
            "Run `just test` before committing. Skipping it hides regressions that only surface on CI.",
            "global",
            &["lesson-log"],
            now - Duration::hours(3),
            Some(-0.5),
            &hash_shared,
        ),
        make_row_with_hash(
            "019ded55-0000-7000-8000-000000000002",
            EntryKind::Lesson,
            "Unrelated lesson that hashes differently",
            "A separate lesson body that pads out the result set without colliding on its content hash prefix.",
            "global",
            &["lesson-log"],
            now - Duration::hours(6),
            Some(-1.0),
            &hash_unique,
        ),
        make_row_with_hash(
            "019dedcc-0000-7000-8000-000000000003",
            EntryKind::Lesson,
            "Stored the same lesson twice (again)",
            "Re-store of the same lesson body. Shares the `deaddeaddeaddead` hash prefix with row one.",
            "global",
            &["lesson-log"],
            now - Duration::days(1),
            Some(-1.5),
            &hash_shared,
        ),
    ];

    let result = RecallResult {
        entries,
        scope_chain: vec!["global".to_owned()],
        scope_hits: vec![("global".to_owned(), 3)],
        token_estimate: 520,
        routing: RecallRouting::Search,
        tier: Some(SearchTier::Exact),
        candidates_before_filter: 5,
        fetch_limit_used: 50,
    };

    let request = RecallRequest {
        query: Some("lesson".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };

    (result, request, now)
}

/// Empty fixture: zero matches, `Search` routing (so the formatter's
/// "show score column" check would fire if any row had a score — none
/// do). Verifies the header still renders and the trailer carries the
/// `no matches` hint.
fn empty_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let now = fixed_now();
    let result = RecallResult {
        entries: Vec::new(),
        scope_chain: vec!["global/project:helioy".to_owned(), "global".to_owned()],
        scope_hits: Vec::new(),
        token_estimate: 0,
        routing: RecallRouting::Search,
        tier: Some(SearchTier::None),
        candidates_before_filter: 0,
        fetch_limit_used: 50,
    };
    let request = RecallRequest {
        query: Some("extremely obscure search phrase".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };
    (result, request, now)
}

#[test]
fn format_recall_view_matches_search_golden() {
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SEARCH,
        "rendered recall search view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_search_prefix_tier_golden() {
    // Prefix tier: header carries `, tier: prefix` and the trailer
    // emits a `# tier: prefix - ...` advisory teaching the caller
    // that the exact implicit-AND query returned zero rows and the
    // cascade advanced to the prefix-match tier.
    let (result, request, now) = search_fixture_with_tier(SearchTier::Prefix);
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SEARCH_PREFIX_TIER,
        "rendered recall search (prefix tier) view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_search_split_or_tier_golden() {
    // SplitOr tier: header carries `, tier: split_or` and the
    // trailer emits a `# tier: split_or - ...` advisory teaching
    // the caller that both the exact and prefix tiers returned
    // zero rows before the OR-joined cascade arm succeeded.
    let (result, request, now) = search_fixture_with_tier(SearchTier::SplitOr);
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SEARCH_SPLIT_OR_TIER,
        "rendered recall search (split_or tier) view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_browse_fallback_golden() {
    let (result, request, now) = browse_fallback_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_BROWSE_FALLBACK,
        "rendered recall browse_fallback view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_empty_golden() {
    let (result, request, now) = empty_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_EMPTY,
        "rendered recall empty view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_dedup_golden() {
    // Intra-response dedup hint: rows 1 and 3 carry the same
    // `deaddeaddeaddead...` content-hash prefix, so row 3 must pick
    // up a `dup_of: 019dedaa` annotation in its trailing YAML
    // comment. Row 2's hash differs entirely, so it renders without
    // annotation. Row 1 is the leader and is also unannotated.
    let (result, request, now) = dedup_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_DEDUP,
        "rendered recall dedup view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
    // Behavioural assertions on top of the byte-for-byte check so any
    // future golden regeneration surfaces the intent if the diff
    // drifts: row 3 carries `dup_of:` pointing at row 1, and no
    // other row does.
    assert!(
        rendered.contains("dup_of: 019dedaa"),
        "row 3 should carry dup_of pointing at row 1:\n{rendered}",
    );
    assert_eq!(
        rendered.matches("dup_of:").count(),
        1,
        "exactly one dup_of annotation expected (row 3 only):\n{rendered}",
    );
}

#[test]
fn format_recall_view_search_fixture_stays_under_2000_bytes() {
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert!(
        rendered.len() <= 2_000,
        "rendered recall search view is {} bytes, exceeds 2000-byte budget:\n{rendered}",
        rendered.len(),
    );
}

#[test]
fn format_recall_view_score_column_omitted_on_non_search_routing() {
    let (result, request, now) = browse_fallback_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    // No normalised score should appear as a leading per-row column.
    // The `0.XX` pattern would sit between the short id and the title.
    assert!(
        !rendered.contains("  0."),
        "browse_fallback rendering should not carry a score column:\n{rendered}",
    );
    assert!(
        !rendered.contains("  1.00 "),
        "browse_fallback rendering should not carry a score column:\n{rendered}",
    );
}

#[test]
fn format_recall_view_header_surfaces_per_kind_and_per_tag_histograms() {
    // ALP-1725 acceptance: the recall header must surface BOTH a
    // per-kind and a per-tag histogram so agents can scan the
    // result-set shape without paging through every row. Regression
    // guard: a prior iteration emitted only `kinds:` because the
    // `tag_histogram` helper existed in `aggregation.rs` but was
    // never wired into `render_header`.
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    // Per-kind histogram: 2 decision + 1 lesson across the 3 rows.
    assert!(
        rendered.contains("\nkinds: decision=2, lesson=1\n"),
        "expected kinds histogram in header:\n{rendered}",
    );
    // Per-tag histogram: `projection` appears on all 3 rows,
    // `snippet` on 2, `edge-case` on 1. Sort order is count desc,
    // then alphabetical within a tie.
    assert!(
        rendered.contains("\ntags: projection=3, snippet=2, edge-case=1\n"),
        "expected tag histogram in header:\n{rendered}",
    );
}

#[test]
fn format_recall_view_empty_fixture_emits_no_matches_hint() {
    let (result, request, now) = empty_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert!(
        rendered.contains("# no matches"),
        "empty rendering should carry the no-matches hint:\n{rendered}",
    );
    // The two-phase `cx_get(id=...)` hint is triggered by oversize
    // rows only, so empty results should never emit it.
    assert!(
        !rendered.contains("cx_get(id="),
        "empty rendering should not emit the cx_get hint:\n{rendered}",
    );
    // Header should still carry query + routing + tokens lines even
    // though the entries block is empty.
    assert!(rendered.contains("query: "), "\n{rendered}");
    assert!(rendered.contains("routing: search"), "\n{rendered}");
    assert!(rendered.contains("tokens: 0"), "\n{rendered}");
    assert!(rendered.contains("entries:\n  []\n"), "\n{rendered}");
}
