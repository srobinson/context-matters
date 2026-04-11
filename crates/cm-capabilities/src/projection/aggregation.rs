//! Aggregation helpers: short ids, relative age, histograms, uniform-key
//! hoisting. Pure, no I/O.
//!
//! Used by the recall/browse YAML formatters to shape result-set headers
//! and row identifiers before rendering.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::hash::Hash;

use chrono::{DateTime, Utc};

/// Default short-id length for entry-row rendering. Used by every view
/// formatter (`browse`, `recall`, `web_view`) so a result set that does
/// not collide on its first 8 bytes renders an 8-char short id.
pub const SHORT_ID_LEN: usize = 8;

/// Extended short-id length used when any two entries in the current
/// result set share their first 8 bytes. Keeps cross-view parity: every
/// formatter widens to the same 12 bytes on a collision.
pub const SHORT_ID_LEN_EXTENDED: usize = 12;

/// First `len` bytes of `id`, safe for multi-byte UTF-8.
///
/// Intended for UUID v7 hex (32 ASCII chars without hyphens), where byte
/// indices are always char boundaries. Falls back to `floor_char_boundary`
/// so arbitrary `&str` inputs never panic. Returns the full string when
/// `len` is greater than or equal to the byte length of `id`.
pub fn short_id(id: &str, len: usize) -> &str {
    let bound = id.floor_char_boundary(len.min(id.len()));
    &id[..bound]
}

/// Whether any two ids in the iterator share their first `len`-byte prefix.
///
/// Used to decide when the default 8-char short id must auto-extend to 12
/// within a single result set. Runs in O(n) with one `HashSet` allocation.
pub fn detect_id_collisions<'a>(ids: impl Iterator<Item = &'a str>, len: usize) -> bool {
    let mut seen: HashSet<&'a str> = HashSet::new();
    for id in ids {
        if !seen.insert(short_id(id, len)) {
            return true;
        }
    }
    false
}

/// Compact human-relative age between two timestamps.
///
/// Selects the largest unit yielding a value of at least 1 and renders it
/// without pluralisation: `<1m`, `Xm`, `Xh`, `Xd`, `Xw`, `Xmo`, `Xy`. Future
/// timestamps (`now < created_at`) collapse to `<1m`.
pub fn relative_age(created_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = now.signed_duration_since(created_at).num_seconds().max(0);
    if secs < 60 {
        return "<1m".to_owned();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    if days < 7 {
        return format!("{days}d");
    }
    if days < 30 {
        return format!("{w}w", w = days / 7);
    }
    if days < 365 {
        return format!("{mo}mo", mo = days / 30);
    }
    format!("{y}y", y = days / 365)
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

/// Format an integer with comma thousands separators (`3420` -> `3,420`).
///
/// Accepts `impl Into<u64>` so callers can pass `u32`, `u64`, or any smaller
/// unsigned type without explicit casts. Used by the recall formatter for
/// token budgets (`u32`) and by the stats formatter for entry counts and
/// byte sizes (`u64`). Pure ASCII; no locale dependency.
pub fn fmt_with_commas(n: impl Into<u64>) -> String {
    let s = n.into().to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_truncates_to_length() {
        let id = "019352b7aae1742fb62ecf4e4d5eac20"; // 32-char hex UUID v7
        assert_eq!(short_id(id, 8), "019352b7");
        assert_eq!(short_id(id, 12), "019352b7aae1");
        assert_eq!(short_id(id, 32), id);
        // `len` longer than the id returns the full string.
        assert_eq!(short_id(id, 100), id);
        // Strings shorter than `len` are returned as-is.
        assert_eq!(short_id("abc", 8), "abc");
    }

    #[test]
    fn detect_id_collisions_flags_duplicates_at_8_chars() {
        // Two distinct ids sharing their first 8 bytes.
        let colliding = [
            "019352b7aae1742fb62ecf4e4d5eac20",
            "019352b7bbf2853ac73fd05f5e6fbd31",
        ];
        assert!(detect_id_collisions(colliding.iter().copied(), 8));
        // At 12 chars the 9th byte differs, so no collision.
        assert!(!detect_id_collisions(colliding.iter().copied(), 12));
        // Wholly distinct ids never collide.
        let distinct = [
            "019352b7aae1742fb62ecf4e4d5eac20",
            "01ff11223344556677889900aabbccdd",
        ];
        assert!(!detect_id_collisions(distinct.iter().copied(), 8));
        // Empty iterator is vacuously collision-free.
        let empty: [&str; 0] = [];
        assert!(!detect_id_collisions(empty.iter().copied(), 8));
    }

    #[test]
    fn relative_age_selects_largest_unit() {
        use chrono::Duration;
        let now: DateTime<Utc> = "2026-04-11T12:00:00Z".parse().unwrap();
        let age = |secs: i64| relative_age(now - Duration::seconds(secs), now);

        assert_eq!(age(0), "<1m");
        assert_eq!(age(59), "<1m");
        assert_eq!(age(60), "1m");
        assert_eq!(age(61), "1m");
        assert_eq!(age(59 * 60), "59m");
        assert_eq!(age(61 * 60), "1h");
        assert_eq!(age(23 * 3600), "23h");
        assert_eq!(age(25 * 3600), "1d");
        assert_eq!(age(6 * 86400), "6d");
        assert_eq!(age(7 * 86400), "1w");
        assert_eq!(age(8 * 86400), "1w");
        assert_eq!(age(14 * 86400), "2w");
        assert_eq!(age(30 * 86400), "1mo");
        assert_eq!(age(60 * 86400), "2mo");
        assert_eq!(age(365 * 86400), "1y");
        assert_eq!(age(730 * 86400), "2y");
        // Future timestamps clamp to "<1m".
        assert_eq!(relative_age(now + Duration::seconds(60), now), "<1m");
    }

    #[test]
    fn hoist_uniform_returns_some_when_all_equal() {
        struct Item {
            kind: &'static str,
        }
        let items = vec![
            Item { kind: "fact" },
            Item { kind: "fact" },
            Item { kind: "fact" },
        ];
        assert_eq!(hoist_uniform(&items, |i| i.kind), Some("fact"));
        // Single-element slice hoists its one value.
        assert_eq!(hoist_uniform(&items[..1], |i| i.kind), Some("fact"));
        // Empty slice returns None.
        let empty: Vec<Item> = vec![];
        assert!(hoist_uniform(&empty, |i| i.kind).is_none());
    }

    #[test]
    fn hoist_uniform_returns_none_on_mixed() {
        struct Item {
            kind: &'static str,
        }
        let items = vec![
            Item { kind: "fact" },
            Item { kind: "fact" },
            Item { kind: "decision" },
        ];
        assert_eq!(hoist_uniform(&items, |i| i.kind), None);
    }

    #[test]
    fn kind_histogram_sorts_by_descending_count_then_alphabetical() {
        struct Item {
            kind: &'static str,
        }
        let items = vec![
            Item { kind: "fact" },
            Item { kind: "decision" },
            Item { kind: "fact" },
            Item { kind: "lesson" },
            Item { kind: "decision" },
            Item { kind: "fact" },
        ];
        let hist = kind_histogram(&items, |i| i.kind);
        assert_eq!(hist.get("fact"), Some(&3));
        assert_eq!(hist.get("decision"), Some(&2));
        assert_eq!(hist.get("lesson"), Some(&1));
        assert_eq!(hist.len(), 3);

        // Formatter-side sort: descending count, alphabetical tiebreak.
        let mut sorted: Vec<(&str, usize)> = hist.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        assert_eq!(sorted, vec![("fact", 3), ("decision", 2), ("lesson", 1)]);
    }

    #[test]
    fn render_histogram_sorts_by_count_desc_then_alpha() {
        let mut hist = BTreeMap::new();
        hist.insert("fact".to_owned(), 2);
        hist.insert("decision".to_owned(), 2);
        hist.insert("lesson".to_owned(), 3);
        assert_eq!(render_histogram(&hist), "lesson=3, decision=2, fact=2");
    }

    #[test]
    fn render_histogram_empty_is_empty_string() {
        let hist: BTreeMap<String, usize> = BTreeMap::new();
        assert_eq!(render_histogram(&hist), "");
    }

    #[test]
    fn fmt_with_commas_inserts_thousands_separators() {
        assert_eq!(fmt_with_commas(0_u32), "0");
        assert_eq!(fmt_with_commas(42_u32), "42");
        assert_eq!(fmt_with_commas(999_u32), "999");
        assert_eq!(fmt_with_commas(1_000_u32), "1,000");
        assert_eq!(fmt_with_commas(3_420_u32), "3,420");
        assert_eq!(fmt_with_commas(1_234_567_u32), "1,234,567");
        assert_eq!(fmt_with_commas(10_000_000_u32), "10,000,000");
        // Widening from u32 and direct u64 must produce the same output.
        assert_eq!(fmt_with_commas(1_234_567_u64), "1,234,567");
        assert_eq!(fmt_with_commas(u64::from(1_234_567_u32)), "1,234,567");
    }

    #[test]
    fn tag_histogram_counts_each_tag_per_entry() {
        struct Item {
            tags: Vec<String>,
        }
        let items = vec![
            Item {
                tags: vec!["rust".into(), "sqlite".into()],
            },
            Item {
                tags: vec!["rust".into(), "mcp".into()],
            },
            Item {
                tags: vec!["rust".into()],
            },
        ];
        let hist = tag_histogram(&items, |i| i.tags.as_slice());
        assert_eq!(hist.get("rust"), Some(&3));
        assert_eq!(hist.get("sqlite"), Some(&1));
        assert_eq!(hist.get("mcp"), Some(&1));
        assert_eq!(hist.len(), 3);
    }
}
