use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use cm_capabilities::recall::{RecallRequest, RecallResult, RecallRouting, SearchTier};
use cm_core::EntryKind;

use super::support::{fixed_now, make_row, make_row_with_hash};

/// `Search` routing fixture: three rows, all carry a raw BM25 score,
/// mixed across kinds and scopes so the header histograms exercise
/// both code paths. The raw scores (-3.47, -1.12, -0.88) match the
/// values used in the normalise_bm25 unit test, so the formatter
/// output's score column directly reflects the test documented
/// normalisation math (1.00, 0.09, 0.00).
pub(crate) fn search_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
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
        relation_counts: HashMap::new(),
        advisories: Vec::new(),
    };

    let request = RecallRequest {
        query: Some("snippet strategy".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };

    (result, request, now)
}

/// Cascade tier override on top of [`search_fixture`]. Used by the
/// prefix and split_or tier snapshot tests so the three tier
/// renderings stay byte identical except for the header suffix and
/// trailing advisory. Mutating the tier in place keeps the row data
/// and scores in one place and means any future change to
/// `search_fixture` automatically propagates to every tier test.
pub(crate) fn search_fixture_with_tier(
    tier: SearchTier,
) -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let (mut result, request, now) = search_fixture();
    result.tier = Some(tier);
    (result, request, now)
}

/// `BrowseFallback` routing fixture: two rows, `score` is `None` on
/// every row because no FTS rank was computed, so the formatter must
/// skip the score column entirely. No query was supplied. The trailer
/// uses the browse fallback advisory.
pub(crate) fn browse_fallback_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
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
        relation_counts: HashMap::new(),
        advisories: Vec::new(),
    };

    let request = RecallRequest {
        query: None,
        limit: 50,
        max_tokens: None,
        ..Default::default()
    };

    (result, request, now)
}

/// Dedup fixture: three rows where rows 1 and 3 carry the same BLAKE3
/// hash 16 char prefix while row 2 is genuinely distinct. The expected
/// rendering annotates row 3 with `dup_of: <row 1 short id>` on its
/// trailing YAML comment and leaves the other two rows untouched.
pub(crate) fn dedup_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let now = fixed_now();
    // Two 16 char hex prefixes padded out to a full 64 char hash. The
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
        relation_counts: HashMap::new(),
        advisories: Vec::new(),
    };

    let request = RecallRequest {
        query: Some("lesson".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };

    (result, request, now)
}

/// Rels fixture: three rows where rows 1 and 2 have populated
/// `relation_counts` entries (3 and 1 outgoing edges respectively)
/// while row 3 is absent from the map. The expected rendering
/// annotates rows 1 and 2 with `rels: N` on their trailing YAML
/// comment and leaves row 3 untouched. Each row carries a unique
/// content hash so the dedup pass never fires in parallel.
pub(crate) fn rels_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
    let now = fixed_now();
    let row1_id =
        Uuid::parse_str("019de1aa-0000-7000-8000-000000000001").expect("test fixture uuid parses");
    let row2_id =
        Uuid::parse_str("019de155-0000-7000-8000-000000000002").expect("test fixture uuid parses");
    let entries = vec![
        make_row(
            "019de1aa-0000-7000-8000-000000000001",
            EntryKind::Decision,
            "Adopt new storage engine",
            "Rationale for adopting the new storage engine. The graph \
             touches three downstream decisions that cite this entry.",
            "global",
            &["decision-log"],
            now - Duration::hours(2),
            Some(-1.5),
        ),
        make_row(
            "019de155-0000-7000-8000-000000000002",
            EntryKind::Fact,
            "Supporting benchmark result",
            "A supporting benchmark figure that one upstream decision \
             in the graph cites for its adoption rationale.",
            "global",
            &["bench"],
            now - Duration::hours(6),
            Some(-2.0),
        ),
        make_row(
            "019de1cc-0000-7000-8000-000000000003",
            EntryKind::Lesson,
            "Standalone lesson with no graph edges",
            "Isolated lesson whose row renders without any trailing \
             rels annotation, because no edges point in or out.",
            "global",
            &["lesson-log"],
            now - Duration::days(1),
            Some(-0.5),
        ),
    ];

    let mut relation_counts: HashMap<Uuid, u32> = HashMap::new();
    relation_counts.insert(row1_id, 3);
    relation_counts.insert(row2_id, 1);

    let result = RecallResult {
        entries,
        scope_chain: vec!["global".to_owned()],
        scope_hits: vec![("global".to_owned(), 3)],
        token_estimate: 520,
        routing: RecallRouting::Search,
        tier: Some(SearchTier::Exact),
        candidates_before_filter: 5,
        fetch_limit_used: 50,
        relation_counts,
        advisories: Vec::new(),
    };

    let request = RecallRequest {
        query: Some("storage engine".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };

    (result, request, now)
}

/// Empty fixture: zero matches, `Search` routing. Verifies the header
/// still renders and the trailer carries the `no matches` hint.
pub(crate) fn empty_fixture() -> (RecallResult, RecallRequest, DateTime<Utc>) {
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
        relation_counts: HashMap::new(),
        advisories: Vec::new(),
    };
    let request = RecallRequest {
        query: Some("extremely obscure search phrase".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };
    (result, request, now)
}
