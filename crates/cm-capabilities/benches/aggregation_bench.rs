//! ALP-1762 scaffold for the recall/browse aggregation hot path.
//!
//! Aggregation helpers live in `crates/cm-capabilities/src/projection/`
//! and produce the histogram + faceted-drill-down hints (ALP-1758) that
//! the recall/browse headers surface. Each aggregation walks the row
//! list once per response and is the next-most-expensive step after
//! the snippet pass. ALP-1763 will fixture realistic row counts (10,
//! 50, 200) and report linear scaling. Placeholder until then.

use criterion::{Criterion, criterion_group, criterion_main};

fn placeholder(c: &mut Criterion) {
    c.bench_function("aggregation/placeholder", |b| b.iter(|| 1_u64 + 1));
}

criterion_group!(benches, placeholder);
criterion_main!(benches);
