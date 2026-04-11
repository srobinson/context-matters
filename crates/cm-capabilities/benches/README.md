# cm-capabilities benchmarks

Criterion benches for the four hot paths in `cm-capabilities`. Run with:

```sh
just bench
# or
cargo bench -p cm-capabilities
```

## Files

| Bench                       | Targets                                                          |
| --------------------------- | ---------------------------------------------------------------- |
| `smart_snippet_bench.rs`    | `projection::snippet::smart_snippet` — once per recall/browse row |
| `normalise_bm25_bench.rs`   | `projection::score::normalise_bm25` — once per recall row        |
| `aggregation_bench.rs`      | recall/browse header histograms + faceted drill-down hints       |
| `format_views_bench.rs`     | `format_*_view` YAML emitters — once per read-tool response      |

## Baselines

Baseline numbers live in **commit messages**, not in this directory.
A README full of stale numbers rots faster than the code. When a
worker measures and lands a meaningful improvement (or regression),
they record the before/after in the commit message that touches the
hot path. `cargo bench -p cm-capabilities -- --save-baseline name`
saves a local baseline under `target/criterion/`; do not commit it.

## Out of scope

* CI-time regression gating (manual `just bench` for now)
* Allocation profiling — pull in `dhat-rs` if a bench surfaces an
  allocation hotspot worth investigating
* Comparison against a "before" branch — use `--baseline name` after
  saving from main, do not bake assumptions into the bench files
