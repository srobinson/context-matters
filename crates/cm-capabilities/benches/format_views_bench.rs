//! ALP-1762 scaffold for the YAML format-views hot path.
//!
//! Format-view helpers live in `crates/cm-capabilities/src/projection/`
//! (recall_view.rs, browse_view.rs, get_view.rs, stats_view.rs) and
//! are the final stage that turns a typed projection into the YAML
//! text channel of the dual-channel envelope (ALP-1760). Called once
//! per response. ALP-1763 will benchmark each format_view function
//! against deterministic fixtures matching the snapshot tests'
//! shape, and report bytes/iter so wire-cap regressions surface
//! quickly. Placeholder until then.

use criterion::{Criterion, criterion_group, criterion_main};

fn placeholder(c: &mut Criterion) {
    c.bench_function("format_views/placeholder", |b| b.iter(|| 1_u64 + 1));
}

criterion_group!(benches, placeholder);
criterion_main!(benches);
