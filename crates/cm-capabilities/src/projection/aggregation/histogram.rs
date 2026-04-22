use std::collections::BTreeMap;
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

/// Render a `{key: count}` histogram as `key=count` pairs joined by `, `,
/// sorted by count descending with alphabetical tiebreak.
///
/// Shared by the browse and recall formatters for the `kinds:`, `scope:`,
/// and other histogram header lines. The descending-count convention
/// surfaces dominant categories first and matches the expectation exercised
/// by `kind_histogram_sorts_by_descending_count_then_alphabetical`.
pub fn render_histogram(hist: &BTreeMap<String, usize>) -> String {
    let mut sorted: Vec<(&String, &usize)> = hist.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let mut out = String::with_capacity(hist.len() * 16);
    for (i, (k, v)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(&mut out, "{k}={v}");
    }
    out
}
