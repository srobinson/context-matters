//! ALP-1762 scaffold for the `normalise_bm25` hot path.
//!
//! `normalise_bm25` lives in `crates/cm-capabilities/src/projection/score.rs`
//! and folds the FTS5 BM25 raw score into the [0, 1] confidence value
//! that recall rows surface. Called once per row in every `cx_recall`
//! response. ALP-1763 will benchmark the realistic input range
//! (typical BM25 magnitudes for the cx_* corpus) and assert the inner
//! loop is allocation-free. Placeholder until then.

use criterion::{Criterion, criterion_group, criterion_main};

fn placeholder(c: &mut Criterion) {
    c.bench_function("normalise_bm25/placeholder", |b| b.iter(|| 1_u64 + 1));
}

criterion_group!(benches, placeholder);
criterion_main!(benches);
