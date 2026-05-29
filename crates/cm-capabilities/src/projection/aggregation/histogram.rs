use std::collections::BTreeMap;
use std::fmt::Display;
use std::fmt::Write as _;
use std::hash::Hash;

/// If every item in `items` maps to the same key, return `Some(key)`.
/// Otherwise, and on an empty slice, return `None`.
///
/// Used to hoist a uniform constant (a common `kind` or `scope_path`) out
/// of each row in a result set and into the response header.
pub fn hoist_uniform<T, K: Eq + Hash>(items: &[T], key: impl Fn(&T) -> K) -> Option<K> {
    let mut iter = items.iter();
    let first = key(iter.next()?);
    for item in iter {
        if key(item) != first {
            return None;
        }
    }
    Some(first)
}

/// Private string-frequency core shared by `kind_histogram` and
/// `scope_histogram`.
fn count_str<T>(items: &[T], key: impl Fn(&T) -> &str) -> BTreeMap<String, usize> {
    let mut map: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        *map.entry(key(item).to_owned()).or_insert(0) += 1;
    }
    map
}

/// Count entries grouped by `kind`.
///
/// Returned as a `BTreeMap` so iteration is deterministic (alphabetical by
/// key); the downstream formatter re-sorts by count descending with
/// alphabetical tiebreak when rendering the histogram.
pub fn kind_histogram<T>(items: &[T], kind: impl Fn(&T) -> &str) -> BTreeMap<String, usize> {
    count_str(items, kind)
}

/// Count entries grouped by `scope_path`. See `kind_histogram` for sort
/// notes.
pub fn scope_histogram<T>(items: &[T], scope: impl Fn(&T) -> &str) -> BTreeMap<String, usize> {
    count_str(items, scope)
}

/// Count tag occurrences across `items`. Each tag on each entry contributes
/// one to its tag's bucket; an entry with three tags increments three
/// different buckets.
pub fn tag_histogram<T>(items: &[T], tags: impl Fn(&T) -> &[String]) -> BTreeMap<String, usize> {
    let mut map: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        for tag in tags(item) {
            *map.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    map
}

/// Project a count map into a `Vec` ordered by count descending, breaking
/// ties alphabetically by key.
///
/// This is the canonical shape for histogram fields on the JSON/web view
/// and MCP structured-output structs. A JSON object carries no key order,
/// and the MCP `dual_response` path runs values through
/// `serde_json::to_value`, which (absent the `preserve_order` feature)
/// rebuilds objects as a `BTreeMap` and discards any insertion order. An
/// ordered `Vec` of `[key, count]` pairs survives every serialization path
/// intact, so dominant categories stay first on the wire. The ordering
/// matches the YAML [`render_histogram`] text surface.
pub fn count_desc_vec<V: Ord>(hist: BTreeMap<String, V>) -> Vec<(String, V)> {
    let mut sorted: Vec<(String, V)> = hist.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

/// [`count_desc_vec`] specialised to the `usize`-counted histograms the
/// recall/browse/search pipelines produce, widening counts to `u32` for the
/// web view and MCP structured-output structs.
///
/// The `u32` width keeps ts-rs projecting the field as `[string, number]`
/// rather than `[string, bigint]`. Counts are bounded by the per-slice
/// result limit (`MAX_LIMIT`), well under `u32::MAX`, so the cast is
/// lossless. Sorts before casting so the single [`count_desc_vec`]
/// comparator stays the only ordering rule.
pub fn count_desc_vec_u32(hist: BTreeMap<String, usize>) -> Vec<(String, u32)> {
    count_desc_vec(hist)
        .into_iter()
        .map(|(k, c)| (k, c as u32))
        .collect()
}

/// Format an already-ordered slice of `(key, count)` pairs as `key=count`
/// joined by `, `. Pairs render in the order given.
///
/// The single formatting primitive for every histogram header line. The
/// search and web view structs hold a pre-sorted [`count_desc_vec`] and
/// render through this directly; [`render_histogram`] funnels `BTreeMap`
/// callers through it after sorting, so text and structured surfaces share
/// one format path.
pub fn render_pairs<K: Display, V: Display>(pairs: &[(K, V)]) -> String {
    let mut out = String::with_capacity(pairs.len() * 16);
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(&mut out, "{k}={v}");
    }
    out
}

/// Render a `{key: count}` map as `key=count` pairs joined by `, `, sorted
/// by count descending with alphabetical tiebreak.
///
/// Used by the browse and recall YAML formatters for the `kinds:`, `tags:`,
/// `scope:`, and `scope_hits:` header lines, where the source is still a
/// `BTreeMap`. Generic over the count type so `usize` and `u32` maps render
/// through one path. Sorts via [`count_desc_vec`] and formats via
/// [`render_pairs`], so it cannot drift from the pre-ordered `Vec` the
/// search/web view structs render. Histograms are tiny (bounded by the
/// result-set kind/tag cardinality), so the clone is negligible.
pub fn render_histogram<V: Ord + Clone + Display>(hist: &BTreeMap<String, V>) -> String {
    render_pairs(&count_desc_vec(hist.clone()))
}
