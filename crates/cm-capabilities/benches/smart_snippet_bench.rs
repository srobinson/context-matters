//! ALP-1762 scaffold for the `smart_snippet` hot path.
//!
//! `smart_snippet` lives in `crates/cm-capabilities/src/projection/snippet.rs`
//! and is called once per row in every `cx_recall`/`cx_browse` response.
//! ALP-1763 will replace `placeholder` with realistic fixtures (mixed
//! short/long bodies, varying query-match positions, the `Bracketed`
//! highlight style from ALP-1750) and report bytes/iter via
//! `Throughput::Bytes`. The placeholder exists so the Cargo wiring
//! verifies before the body lands.

use criterion::{Criterion, criterion_group, criterion_main};

fn placeholder(c: &mut Criterion) {
    c.bench_function("smart_snippet/placeholder", |b| b.iter(|| 1_u64 + 1));
}

criterion_group!(benches, placeholder);
criterion_main!(benches);
