//! Benchmark the recall/browse aggregation helpers that run once per
//! response after the row slice is assembled and before the YAML
//! formatter walks it:
//!
//!   * `kind_histogram` / `scope_histogram` / `tag_histogram` — the
//!     three passes `format_browse_view` and `format_recall_view`
//!     call on the row slice to build the header's per-facet counts.
//!   * `compute_dedup_hints` (ALP-1755) — intra-response dedup using
//!     a 16-char `content_hash` prefix.
//!   * `compute_drill_down_hint` (ALP-1758) — faceted dominance check
//!     against the 60% threshold on the kind and tag histograms.
//!
//! Fixtures are hand-built `Entry` values: no store, no clock, no
//! `rand`, no IO. UUIDs are derived from a counter via
//! `Uuid::from_u128` so the bench is byte-stable across runs.
//!
//! The histogram group covers 20-row and 200-row fixtures to expose
//! linear scaling; the dedup and drill-down groups pin on a 20-row
//! fixture because they model the common-case recall response size
//! and their cost floor is `HashMap` allocation rather than row
//! count.

use std::collections::BTreeMap;
use std::hint::black_box;

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::projection::{
    compute_dedup_hints, compute_drill_down_hint, kind_histogram, scope_histogram, tag_histogram,
};
use cm_core::{Entry, EntryKind, EntryMeta, ScopePath};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

/// Deterministic reference `now` shared by every fixture row. Matches
/// the recall/browse snapshot tests so the bench and the tests use
/// the same pinned instant.
fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

/// Build an `Entry` with the supplied discriminators and a
/// counter-derived UUID. Called from every fixture below; keeps the
/// per-fixture code compact.
fn make_entry(idx: usize, kind: EntryKind, scope: &str, tags: &[&str], hash: String) -> Entry {
    Entry {
        id: Uuid::from_u128(idx as u128 + 1),
        scope_path: ScopePath::parse(scope).expect("bench fixture scope parses"),
        kind,
        title: format!("row-{idx}"),
        body: "body".to_owned(),
        content_hash: hash,
        meta: Some(EntryMeta {
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            ..Default::default()
        }),
        created_by: "agent:claude-code".to_owned(),
        created_at: fixed_now(),
        updated_at: fixed_now(),
        superseded_by: None,
    }
}

/// Build `n` rows with varied kinds, scopes, and tags so the
/// histogram passes walk realistic bucket counts instead of
/// collapsing onto a single key.
fn diverse_fixture(n: usize) -> Vec<Entry> {
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
    (0..n)
        .map(|i| {
            let kind = KINDS[i % KINDS.len()];
            let scope = SCOPES[i % SCOPES.len()];
            // 1..=3 tags per row, rotating through the palette so
            // `tag_histogram` does real work on every row.
            let tag_count = 1 + (i % 3);
            let row_tags: Vec<&str> = (0..tag_count)
                .map(|j| TAG_PALETTE[(i + j) % TAG_PALETTE.len()])
                .collect();
            let hash = format!("{:0>64x}", i as u128);
            make_entry(i, kind, scope, &row_tags, hash)
        })
        .collect()
}

/// 20-row fixture with a dominant kind: 14/20 (= 70%) rows carry
/// `EntryKind::Observation`, clearing the 60% `DRILL_DOWN_THRESHOLD`
/// so `compute_drill_down_hint` walks the full kind-wins branch.
fn dominant_kind_fixture() -> Vec<Entry> {
    (0..20)
        .map(|i| {
            let kind = if i < 14 {
                EntryKind::Observation
            } else if i < 17 {
                EntryKind::Decision
            } else {
                EntryKind::Lesson
            };
            let scope = if i % 2 == 0 {
                "global"
            } else {
                "global/project:helioy"
            };
            let hash = format!("{:0>64x}", (i as u128) + 100);
            make_entry(i, kind, scope, &["lesson-log"], hash)
        })
        .collect()
}

/// 20-row fixture with three `content_hash` duplicate pairs. Rows
/// 0/1, 2/3, and 4/5 share 16-char hash prefixes (three leaders,
/// three duplicates); rows 6..20 are unique. Matches the issue's
/// "3 dup pairs" language.
fn dedup_fixture() -> Vec<Entry> {
    const PAIR_PREFIXES: [&str; 3] = ["deaddeaddeaddead", "cafecafecafecafe", "beefbeefbeefbeef"];
    (0..20)
        .map(|i| {
            let hash = if i < 6 {
                // Leader and duplicate of each pair share a 16-char
                // prefix; the remaining 48 chars diverge so the full
                // hashes remain unique even though the dedup key
                // (first 16 chars) collides.
                let pair = &PAIR_PREFIXES[i / 2];
                format!("{pair}{:0>48x}", i as u128)
            } else {
                format!("{:0>64x}", (i as u128) + 200)
            };
            make_entry(i, EntryKind::Observation, "global", &["lesson-log"], hash)
        })
        .collect()
}

fn bench_histograms(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregation/histograms");
    for n in [20usize, 200] {
        let fixture = diverse_fixture(n);
        group.bench_with_input(
            BenchmarkId::new("kind_histogram", n),
            &fixture,
            |b, fixture| {
                b.iter(|| kind_histogram(black_box(fixture), |e: &Entry| e.kind.as_str()));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("scope_histogram", n),
            &fixture,
            |b, fixture| {
                b.iter(|| scope_histogram(black_box(fixture), |e: &Entry| e.scope_path.as_str()));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("tag_histogram", n),
            &fixture,
            |b, fixture| {
                b.iter(|| {
                    tag_histogram(black_box(fixture), |e: &Entry| {
                        e.meta.as_ref().map(|m| m.tags.as_slice()).unwrap_or(&[])
                    })
                });
            },
        );
    }
    group.finish();
}

fn bench_dedup(c: &mut Criterion) {
    let fixture = dedup_fixture();
    let row_refs: Vec<&Entry> = fixture.iter().collect();
    let mut group = c.benchmark_group("aggregation/dedup");
    group.bench_function("compute_dedup_hints/20_with_3_pairs", |b| {
        b.iter(|| compute_dedup_hints(black_box(row_refs.as_slice())));
    });
    group.finish();
}

fn bench_drill_down(c: &mut Criterion) {
    let fixture = dominant_kind_fixture();
    let kinds: BTreeMap<String, usize> = kind_histogram(&fixture, |e: &Entry| e.kind.as_str());
    let tags: BTreeMap<String, usize> = tag_histogram(&fixture, |e: &Entry| {
        e.meta.as_ref().map(|m| m.tags.as_slice()).unwrap_or(&[])
    });
    let total = fixture.len();
    let mut group = c.benchmark_group("aggregation/drill_down");
    group.bench_function("compute_drill_down_hint/20_dominant_kind", |b| {
        b.iter(|| compute_drill_down_hint(black_box(&kinds), black_box(&tags), black_box(total)));
    });
    group.finish();
}

criterion_group!(histograms, bench_histograms);
criterion_group!(dedup, bench_dedup);
criterion_group!(drill_down, bench_drill_down);
criterion_main!(histograms, dedup, drill_down);
