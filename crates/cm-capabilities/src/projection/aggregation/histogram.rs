use std::collections::BTreeMap;
use std::fmt::Display;
use std::fmt::Write as _;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One ordered count bucket in structured projection headers.
///
/// `bucket` carries the facet value for the containing field, such as a
/// kind, tag, or scope path. The array order is the meaningful ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CountBucket {
    pub bucket: String,
    pub count: u32,
}

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

/// Project a count map into pairs ordered by count descending, breaking
/// ties alphabetically by key.
///
/// This is the shared ordering rule for text rendering and structured
/// projection headers. A JSON object carries no key order, and the MCP
/// `dual_response` path runs values through `serde_json::to_value`, which
/// (absent the `preserve_order` feature) rebuilds objects as a `BTreeMap`
/// and discards insertion order. Ordered arrays survive every serialization
/// path intact, so dominant categories stay first on the wire.
pub fn count_desc_vec<V: Ord>(hist: BTreeMap<String, V>) -> Vec<(String, V)> {
    let mut sorted: Vec<(String, V)> = hist.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

/// Convert already-ordered count pairs into named structured buckets.
///
/// The `u32` width keeps ts-rs projecting `count` as `number` rather than
/// `bigint`. Counts are bounded by result limits, well under `u32::MAX`.
pub fn count_buckets(pairs: impl IntoIterator<Item = (String, usize)>) -> Vec<CountBucket> {
    pairs
        .into_iter()
        .map(|(bucket, count)| CountBucket {
            bucket,
            count: u32::try_from(count).expect("projection count fits in u32"),
        })
        .collect()
}

/// [`count_desc_vec`] specialised to structured projection headers.
pub fn count_desc_buckets(hist: BTreeMap<String, usize>) -> Vec<CountBucket> {
    count_buckets(count_desc_vec(hist))
}

fn render_key_values<'a, I, K, V>(pairs: I) -> String
where
    I: IntoIterator<Item = (&'a K, &'a V)>,
    K: Display + 'a,
    V: Display + 'a,
{
    let mut out = String::new();
    for (i, (k, v)) in pairs.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(&mut out, "{k}={v}");
    }
    out
}

/// Format an already-ordered slice of `(key, count)` pairs as `key=count`
/// joined by `, `. Pairs render in the order given.
///
/// `scope_hits` still uses tuple pairs in the core recall result, so this
/// remains the tuple formatting adapter. Structured headers use
/// [`render_buckets`].
pub fn render_pairs<K: Display, V: Display>(pairs: &[(K, V)]) -> String {
    render_key_values(pairs.iter().map(|(k, v)| (k, v)))
}

/// Format ordered structured buckets as `bucket=count` joined by `, `.
pub fn render_buckets(buckets: &[CountBucket]) -> String {
    render_key_values(buckets.iter().map(|bucket| (&bucket.bucket, &bucket.count)))
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
