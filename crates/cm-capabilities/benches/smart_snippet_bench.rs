//! Benchmark `cm_capabilities::projection::smart_snippet`, called once
//! per row by every `cx_recall` and `cx_browse` response.
//!
//! Exercises the three bodies the projection layer actually sees in
//! production — a plain prose body with no frontmatter, a short-
//! frontmatter body, and a long-frontmatter body — each in both the
//! centring (query matched) and start-of-body (no query) paths. The
//! frontmatter paths sit on a different codepath from the plain body
//! because `smart_snippet` calls `strip_yaml_frontmatter` plus
//! `strip_leading_markdown_heading` before the window pass, so their
//! throughput diverges from the no-frontmatter case on short inputs.
//!
//! Throughput is reported in bytes per iteration via `Throughput::Bytes`
//! so a future regression surfaces as a bytes/sec delta rather than a
//! raw wall-clock number. Fixtures are hand-built `&str` constants: no
//! `rand`, no clock, no IO, no DB access.

use cm_capabilities::projection::{HighlightStyle, SNIPPET_MAX_BYTES, smart_snippet};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// Plain prose body, roughly 2 KB, no frontmatter and no leading
/// markdown heading. Exercises the pure window pass: no strip,
/// `first_query_match_position` scan across the full body, then
/// `snippet_around` + optional `insert_highlights`. Repetitive on
/// purpose so the query term can be found anywhere without biasing
/// the window location.
const PROSE_NO_FRONTMATTER: &str = "\
The byte-prefix snippet drops mid-word when the body contains multi-byte \
characters because `str::get(..n)` slices at raw byte offsets, so the first \
stable iteration used `floor_char_boundary` to round down to the nearest \
character. That fix held until the query-centred variant landed: then the \
snippet had to be centred on the first query-term match instead of the \
start of the stripped body, and the centring walk had to extend the left \
edge backwards to the nearest preceding space so tokens would not be cut. \
A word-boundary walk on the right edge keeps tokens whole without ever \
panicking on multi-byte UTF-8. The prose here is intentionally \
repetitive so the benchmark's query term — snippet strategy — appears \
near the middle of the body and the centring path is exercised on every \
run. Without the repetition the query match would land at a fixed byte \
offset and the `first_query_match_position` scan would short-circuit \
uniformly, masking the characterisation the benchmark is meant to \
surface. The snippet strategy used here keeps tokens whole without \
ever panicking on multi-byte UTF-8, which is the property the unit \
tests guard. Additional prose fills the body out toward two kilobytes \
so the throughput number reflects realistic cx_recall row bodies \
rather than the trivial-short-input codepath.";

/// ~200 byte YAML frontmatter block followed by the same prose body.
/// Exercises the strip-then-window codepath with a short strip that
/// returns quickly from `strip_yaml_frontmatter`. The prose after the
/// fence is short so the bench measures mostly the strip + scan cost,
/// not the window extraction.
const PROSE_SHORT_FRONTMATTER: &str = "\
---
title: Short frontmatter fixture
tags: [snippet, projection, bench]
scope: global/project:helioy
kind: decision
author: agent:claude-code
---

The snippet strategy used here keeps tokens whole without ever panicking \
on multi-byte UTF-8, which is the property the unit tests guard. The body \
after the frontmatter fence stays short so the strip-and-scan path is the \
dominant cost in the benchmark numbers.";

/// ~2 KB YAML frontmatter block followed by the same prose body. The
/// body after the fence is prose-heavy so both the strip and the
/// window extraction contribute to the reported wall time. Exercises
/// the worst case for `strip_yaml_frontmatter` — a long multi-line
/// frontmatter that walks line-by-line until the closing `---` fence.
const PROSE_LONG_FRONTMATTER: &str = "\
---
title: Long frontmatter fixture for the centring benchmark
tags:
  - snippet
  - projection
  - bench
  - long-frontmatter
  - alp-1763
scope: global/project:helioy/repo:context-matters
kind: decision
author: agent:claude-code
created_at: 2026-04-11T12:00:00Z
updated_at: 2026-04-11T12:00:00Z
confidence: high
related_decisions:
  - ALP-1746
  - ALP-1747
  - ALP-1748
  - ALP-1749
  - ALP-1750
  - ALP-1751
  - ALP-1752
  - ALP-1753
  - ALP-1754
  - ALP-1755
  - ALP-1756
  - ALP-1757
  - ALP-1758
  - ALP-1759
  - ALP-1760
  - ALP-1761
  - ALP-1762
  - ALP-1763
acceptance_notes: |
  Long frontmatter block that pads the strip-line scan to around two
  kilobytes of non-prose content before the closing fence. The body
  after the fence is held steady across all three fixtures so any
  runtime divergence between the short and long variants is
  attributable to the frontmatter strip, not the window extraction.
worker_context: |
  Fixture authored for ALP-1763. Every field is inline so the bench
  stays deterministic with zero reliance on a clock or rand source.
---

The byte-prefix snippet drops mid-word when the body contains multi-byte \
characters because `str::get(..n)` slices at raw byte offsets, so the first \
stable iteration used `floor_char_boundary` to round down to the nearest \
character. The snippet strategy used here keeps tokens whole without ever \
panicking on multi-byte UTF-8, which is the property the unit tests \
guard. Additional prose fills the body out after the strip so the \
centring pass has realistic input to walk.";

/// Realistic query term that matches in every fixture body, so the
/// centring path runs to completion rather than short-circuiting at
/// the `first_query_match_position` empty-return branch.
const QUERY: &str = "snippet strategy";

fn bench_smart_snippet(c: &mut Criterion) {
    let fixtures: [(&str, &str); 3] = [
        ("no_frontmatter", PROSE_NO_FRONTMATTER),
        ("short_frontmatter", PROSE_SHORT_FRONTMATTER),
        ("long_frontmatter", PROSE_LONG_FRONTMATTER),
    ];

    let mut group = c.benchmark_group("smart_snippet");
    for (label, body) in &fixtures {
        // Throughput reports bytes/sec over the full body so cross-
        // fixture comparisons are apples-to-apples even though the
        // strip pass consumes only the leading region.
        group.throughput(Throughput::Bytes(body.len() as u64));

        // No-query path: start-of-body window, no highlight pass.
        group.bench_with_input(BenchmarkId::new("no_query", label), body, |b, body| {
            b.iter(|| {
                smart_snippet(
                    black_box(body),
                    black_box(None),
                    black_box(HighlightStyle::None),
                    black_box(SNIPPET_MAX_BYTES),
                )
            });
        });

        // Query + bracketed path: full centring scan plus the highlight
        // + truncate-respecting-brackets post-pass (ALP-1750).
        group.bench_with_input(
            BenchmarkId::new("query_bracketed", label),
            body,
            |b, body| {
                b.iter(|| {
                    smart_snippet(
                        black_box(body),
                        black_box(Some(QUERY)),
                        black_box(HighlightStyle::Bracketed),
                        black_box(SNIPPET_MAX_BYTES),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_smart_snippet);
criterion_main!(benches);
