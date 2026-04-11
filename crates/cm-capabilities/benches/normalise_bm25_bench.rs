//! Benchmark `cm_capabilities::projection::normalise_bm25`, the min-max
//! normalisation pass that turns raw SQLite FTS5 `bm25()` scores into
//! the `[0.0, 1.0]` confidence column the recall formatter renders.
//!
//! `normalise_bm25` runs once per `cx_recall` response (not per row)
//! but walks the full score slice three times — two folds for
//! min/max, one iterator for the scaled output — so its cost scales
//! linearly with row count. The issue spec asks for a single
//! deterministic 20-row case; this file adds a handful of smaller
//! and larger sizes via `BenchmarkId` so the linear-scaling property
//! is visible in the criterion report and any future allocator
//! regression surfaces as a sudden sub-linear break.
//!
//! Edge cases:
//!   - The 20-row case exercises the scaled-output path with a
//!     realistic range derived from the values in
//!     `tests/recall_format_tests.rs:134-158`.
//!   - A `uniform` case exercises the `range < EPSILON` early-return
//!     branch at `recall_view.rs:426` that hands back a constant
//!     `1.0` vector instead of a NaN-producing divide.
//!   - An `empty` case exercises the allocation-free `Vec::new()`
//!     early-return for empty inputs.
//!
//! No `rand`, no clock, no IO, no DB access. All fixtures are
//! hand-built `[f32; N]` arrays.

use cm_capabilities::projection::normalise_bm25;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// Deterministic raw BM25 scores modelled on the values in the
/// recall-format snapshot test (`-3.47`, `-1.12`, `-0.88`), extended
/// to 20 slots by interpolating plausibly between them. Covers the
/// range [-3.47, -0.22] so min/max/range are all non-degenerate and
/// the scaled output spans the full `[0.0, 1.0]` window.
const SCORES_20: [f32; 20] = [
    -3.47, -3.10, -2.85, -2.60, -2.35, -2.10, -1.85, -1.60, -1.35, -1.12, -0.98, -0.88, -0.78,
    -0.68, -0.58, -0.48, -0.42, -0.36, -0.30, -0.22,
];

/// 5-row slice carved out of `SCORES_20` for the small-input case.
const SCORES_5: [f32; 5] = [-3.47, -2.35, -1.12, -0.58, -0.22];

/// 50-row fixture: `SCORES_20` tiled two and a half times. Large
/// enough that any per-row allocation would dominate the reported
/// time and small enough that the bench still runs in milliseconds.
const SCORES_50_LEN: usize = 50;

/// 200-row fixture: top of the realistic cx_recall page size.
const SCORES_200_LEN: usize = 200;

/// Uniform slice: exercises the `range.abs() < EPSILON` early-return
/// branch that returns `vec![1.0; scores.len()]` without running the
/// scaling closure.
const SCORES_UNIFORM: [f32; 20] = [-2.5; 20];

fn scores_of_len(len: usize) -> Vec<f32> {
    // Repeat `SCORES_20` to fill the requested length without
    // `rand` or the clock. Deterministic across runs.
    (0..len).map(|i| SCORES_20[i % SCORES_20.len()]).collect()
}

fn bench_normalise_bm25(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalise_bm25");

    let scores_50 = scores_of_len(SCORES_50_LEN);
    let scores_200 = scores_of_len(SCORES_200_LEN);

    for (label, scores) in [
        ("varied/5", SCORES_5.as_slice()),
        ("varied/20", SCORES_20.as_slice()),
        ("varied/50", scores_50.as_slice()),
        ("varied/200", scores_200.as_slice()),
    ] {
        // Throughput reports bytes/sec over the input slice so the
        // linear-scaling property is visible across sizes in the
        // criterion report.
        group.throughput(Throughput::Bytes(std::mem::size_of_val(scores) as u64));
        group.bench_with_input(BenchmarkId::new("varied", label), scores, |b, scores| {
            b.iter(|| normalise_bm25(black_box(scores)));
        });
    }

    // Uniform slice: `range < EPSILON` early return.
    group.bench_function("uniform/20", |b| {
        b.iter(|| normalise_bm25(black_box(&SCORES_UNIFORM)));
    });

    // Empty slice: `Vec::new()` allocation-free early return.
    let empty: [f32; 0] = [];
    group.bench_function("empty", |b| {
        b.iter(|| normalise_bm25(black_box(&empty)));
    });

    group.finish();
}

criterion_group!(benches, bench_normalise_bm25);
criterion_main!(benches);
