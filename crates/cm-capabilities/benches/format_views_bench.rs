//! Benchmark `format_browse_view_at` and `format_recall_view_at`, the
//! terminal YAML-text render stage of the `cx_browse` and `cx_recall`
//! response pipelines (ALP-1725). These two functions are called once
//! per response and produce the `content[0].text` channel of the
//! dual-channel envelope (ALP-1760), so their wall time is the "total
//! render time" number a future wire-bytes-per-second regression
//! will surface first.
//!
//! The fixtures mirror the snapshot tests in
//! `tests/browse_format_tests.rs` and `tests/recall_format_tests.rs`
//! but at 20-row width so the benchmark exercises histogram,
//! dedup-hint, drill-down, and relation-count annotation passes on a
//! realistic cx_recall page size rather than the 3-row snapshot
//! fixtures that pin byte-shape.
//!
//! Throughput is reported in bytes per iteration via `Throughput::Bytes`
//! using the length of the rendered output, so a future regression
//! surfaces as a bytes/sec delta. All fixtures are hand-built: no
//! store, no clock, no `rand`, no IO.

use std::collections::HashMap;
use std::hint::black_box;

use chrono::{DateTime, Duration, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::browse::{BrowseRequest, BrowseResult};
use cm_capabilities::projection::{RecallRow, format_browse_view_at, format_recall_view_at};
use cm_capabilities::recall::{RecallRequest, RecallResult, RecallRouting, SearchTier};
use cm_core::{BrowseSort, Entry, EntryKind, EntryMeta, ScopePath};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

/// Pinned reference `now` matching `tests/{browse,recall}_format_tests.rs`
/// so the rendered `age:` column is deterministic across runs.
fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

/// Row count for the bench fixtures. Matches the issue spec (20-row
/// fixture for `format_browse_view` and `format_recall_view`).
const FIXTURE_SIZE: usize = 20;

const KINDS: [EntryKind; 8] = [
    EntryKind::Fact,
    EntryKind::Decision,
    EntryKind::Preference,
    EntryKind::Lesson,
    EntryKind::Reference,
    EntryKind::Feedback,
    EntryKind::Pattern,
    EntryKind::Observation,
];

const SCOPES: [&str; 4] = [
    "global",
    "global/project:helioy",
    "global/project:helioy/repo:context-matters",
    "global/project:helioy/repo:cm-store",
];

const TAG_PALETTE: [&str; 7] = [
    "projection",
    "snippet",
    "ci",
    "bench",
    "lesson-log",
    "session-log",
    "decision-log",
];

/// Synthesise a single `Entry` for the fixture slice at position `idx`.
/// Deterministic: id is derived from `idx`, timestamps from
/// `fixed_now() - idx hours`, kind/scope/tags rotated through
/// `KINDS` / `SCOPES` / `TAG_PALETTE`.
fn make_entry(idx: usize) -> Entry {
    let kind = KINDS[idx % KINDS.len()];
    let scope = SCOPES[idx % SCOPES.len()];
    let tag_count = 1 + (idx % 3);
    let tags: Vec<String> = (0..tag_count)
        .map(|j| TAG_PALETTE[(idx + j) % TAG_PALETTE.len()].to_owned())
        .collect();
    // Prose body that the smart-snippet pass can walk without
    // short-circuiting on a too-small input. The content varies per
    // row so dedup never fires falsely.
    let body = format!(
        "Row {idx}: the snippet strategy centres the window on the \
         first query-term match. This body is long enough that \
         `smart_snippet` takes the centring codepath rather than \
         returning the full body verbatim, and short enough that the \
         bench stays fast."
    );
    // Unique 64-char content_hash per row so intra-response dedup
    // never fires in the common-case render path.
    let content_hash = format!("{:0>64x}", idx as u128 + 1);
    let updated_at = fixed_now() - Duration::hours(idx as i64);
    Entry {
        id: Uuid::from_u128(idx as u128 + 1),
        scope_path: ScopePath::parse(scope).expect("bench fixture scope parses"),
        kind,
        title: format!("Row {idx}: snippet strategy case study"),
        body,
        content_hash,
        meta: Some(EntryMeta {
            tags,
            ..Default::default()
        }),
        created_by: "agent:claude-code".to_owned(),
        created_at: updated_at,
        updated_at,
        superseded_by: None,
    }
}

/// Build the shared 20-entry slice used by both formatters.
fn diverse_entries() -> Vec<Entry> {
    (0..FIXTURE_SIZE).map(make_entry).collect()
}

/// `BrowseResult` + `BrowseRequest` fixture for `format_browse_view_at`.
/// `has_more` is true and `next_cursor` populated so the pagination
/// trailer renders. `relation_counts` carries a few entries so the
/// `rels: N` annotation pass walks the HashMap lookup branch.
fn browse_fixture() -> (BrowseResult, BrowseRequest) {
    let entries = diverse_entries();
    let mut relation_counts: HashMap<Uuid, u32> = HashMap::new();
    // Three rows carry outgoing relation counts so the rels-annotation
    // branch runs on a realistic sparse population.
    relation_counts.insert(entries[0].id, 3);
    relation_counts.insert(entries[4].id, 1);
    relation_counts.insert(entries[9].id, 2);

    let result = BrowseResult {
        entries,
        total: 247,
        next_cursor: Some("eyJzb3J0IjoicmVjZW50IiwibGFzdCI6ImZvbyJ9".to_owned()),
        has_more: true,
        sort_used: BrowseSort::Recent,
        relation_counts,
    };
    let request = BrowseRequest {
        limit: 50,
        ..Default::default()
    };
    (result, request)
}

/// `RecallResult` + `RecallRequest` fixture for `format_recall_view_at`.
/// Every row carries a synthetic BM25 score so the score column
/// renders and `normalise_bm25` walks the scaled-output branch. The
/// routing is `Search` + `SearchTier::Exact` to exercise the most
/// detailed header/trailer shape.
fn recall_fixture() -> (RecallResult, RecallRequest) {
    let entries: Vec<RecallRow> = diverse_entries()
        .into_iter()
        .enumerate()
        .map(|(i, entry)| RecallRow {
            entry,
            // Scores step from -3.5 toward 0 across the slice so the
            // min/max/range is non-degenerate and the normalisation
            // pass walks the full scaling branch.
            score: Some(-3.5 + (i as f32) * 0.15),
        })
        .collect();

    let mut relation_counts: HashMap<Uuid, u32> = HashMap::new();
    relation_counts.insert(entries[0].entry.id, 3);
    relation_counts.insert(entries[5].entry.id, 1);
    relation_counts.insert(entries[12].entry.id, 2);

    let result = RecallResult {
        entries,
        scope_chain: vec!["global/project:helioy".to_owned(), "global".to_owned()],
        scope_hits: vec![
            ("global/project:helioy".to_owned(), 15),
            ("global".to_owned(), 5),
        ],
        token_estimate: 4_820,
        routing: RecallRouting::Search,
        tier: Some(SearchTier::Exact),
        candidates_before_filter: 62,
        fetch_limit_used: 50,
        relation_counts,
    };
    let request = RecallRequest {
        query: Some("snippet strategy".to_owned()),
        limit: 50,
        max_tokens: Some(8_000),
        ..Default::default()
    };
    (result, request)
}

fn bench_format_browse_view(c: &mut Criterion) {
    let (result, request) = browse_fixture();
    let now = fixed_now();
    // Render once to compute the throughput baseline so the
    // bytes/sec number is stable across runs.
    let rendered_len = format_browse_view_at(&result, &request, now).len() as u64;
    let mut group = c.benchmark_group("format_views");
    group.throughput(Throughput::Bytes(rendered_len));
    group.bench_function("format_browse_view/20_rows", |b| {
        b.iter(|| format_browse_view_at(black_box(&result), black_box(&request), black_box(now)));
    });
    group.finish();
}

fn bench_format_recall_view(c: &mut Criterion) {
    let (result, request) = recall_fixture();
    let now = fixed_now();
    let rendered_len = format_recall_view_at(&result, &request, now).len() as u64;
    let mut group = c.benchmark_group("format_views");
    group.throughput(Throughput::Bytes(rendered_len));
    group.bench_function("format_recall_view/20_rows", |b| {
        b.iter(|| format_recall_view_at(black_box(&result), black_box(&request), black_box(now)));
    });
    group.finish();
}

criterion_group!(benches, bench_format_browse_view, bench_format_recall_view);
criterion_main!(benches);
