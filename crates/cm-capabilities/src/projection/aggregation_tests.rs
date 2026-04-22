use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use cm_core::Entry;
use uuid::Uuid;

use super::*;

#[test]
fn hex_prefix_truncates_to_length() {
    let id = "019352b7aae1742fb62ecf4e4d5eac20"; // 32-char hex UUID v7
    assert_eq!(hex_prefix(id, 8), "019352b7");
    assert_eq!(hex_prefix(id, 12), "019352b7aae1");
    assert_eq!(hex_prefix(id, 32), id);
    // `len` longer than the id returns the full string.
    assert_eq!(hex_prefix(id, 100), id);
    // Strings shorter than `len` are returned as-is.
    assert_eq!(hex_prefix("abc", 8), "abc");
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

/// Minimal `Entry` builder used only by the `compute_dedup_hints`
/// unit tests. Every field other than `id` and `content_hash` is
/// filled with placeholder values because dedup cares only about
/// those two fields; setting anything else risks test coupling to
/// the rest of the `Entry` shape.
fn fixture_entry(id_hex: &str, content_hash: &str) -> Entry {
    use cm_core::{EntryKind, ScopePath};
    let now = Utc::now();
    Entry {
        id: Uuid::parse_str(id_hex).expect("test fixture uuid parses"),
        scope_path: ScopePath::parse("global").expect("global scope parses"),
        kind: EntryKind::Fact,
        title: String::new(),
        body: String::new(),
        content_hash: content_hash.to_owned(),
        meta: None,
        created_by: "test".to_owned(),
        created_at: now,
        updated_at: now,
        superseded_by: None,
    }
}

/// `0`-padded 64-char hex literal from a short unique head. Lets a
/// test spell out a content hash without typing 64 characters.
fn hash(head: &str) -> String {
    assert!(
        head.len() <= 64,
        "dedup test hash head must fit inside 64 hex chars"
    );
    format!("{head:0<64}")
}

#[test]
fn dedup_empty_rows() {
    let rows: Vec<&Entry> = Vec::new();
    assert!(compute_dedup_hints(&rows).is_empty());
}

#[test]
fn dedup_all_unique() {
    let e1 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000001",
        &hash("aaaaaaaaaaaaaaaa"),
    );
    let e2 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000002",
        &hash("bbbbbbbbbbbbbbbb"),
    );
    let e3 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000003",
        &hash("cccccccccccccccc"),
    );
    let rows = vec![&e1, &e2, &e3];
    let map = compute_dedup_hints(&rows);
    assert!(
        map.is_empty(),
        "three distinct content hashes must not flag any dupes: {map:?}"
    );
}

#[test]
fn dedup_one_pair() {
    let e1 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000001",
        &hash("aaaaaaaaaaaaaaaa"),
    );
    let e2 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000002",
        &hash("bbbbbbbbbbbbbbbb"),
    );
    let e3 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000003",
        &hash("aaaaaaaaaaaaaaaa"),
    );
    let rows = vec![&e1, &e2, &e3];
    let map = compute_dedup_hints(&rows);
    assert_eq!(map.len(), 1, "exactly one dupe expected: {map:?}");
    assert_eq!(
        map.get(&e3.id),
        Some(&e1.id),
        "row 3 should map to row 1 leader",
    );
    // Leader and unique row are never keys in the output.
    assert!(!map.contains_key(&e1.id));
    assert!(!map.contains_key(&e2.id));
}

#[test]
fn dedup_triplet() {
    let e1 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000001",
        &hash("aaaaaaaaaaaaaaaa"),
    );
    let e2 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000002",
        &hash("aaaaaaaaaaaaaaaa"),
    );
    let e3 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000003",
        &hash("aaaaaaaaaaaaaaaa"),
    );
    let rows = vec![&e1, &e2, &e3];
    let map = compute_dedup_hints(&rows);
    assert_eq!(map.len(), 2, "two dupes expected against leader: {map:?}");
    // Both later rows map to the first occurrence, not a chain.
    assert_eq!(map.get(&e2.id), Some(&e1.id));
    assert_eq!(map.get(&e3.id), Some(&e1.id));
    assert!(!map.contains_key(&e1.id));
}

#[test]
fn dedup_keys_only_on_first_16_hex_chars() {
    // Two hashes that share the leading 16 hex chars but differ at
    // byte 17 are still treated as duplicates. The prefix compare
    // is the whole point of the cheap dedup pass.
    let e1 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000001",
        &hash("aaaaaaaaaaaaaaaa11"),
    );
    let e2 = fixture_entry(
        "019d8a01-0000-7000-8000-000000000002",
        &hash("aaaaaaaaaaaaaaaa22"),
    );
    let rows = vec![&e1, &e2];
    let map = compute_dedup_hints(&rows);
    assert_eq!(map.get(&e2.id), Some(&e1.id));
}

/// Builder for [`compute_drill_down_hint`] inputs. Each test passes
/// `(kind, count)` and `(tag, count)` slices that are converted into
/// the `BTreeMap<String, usize>` shape the function expects, plus the
/// row total separately so a test can probe the threshold edge
/// without re-summing the histogram.
fn drill_down_inputs(
    kinds: &[(&str, usize)],
    tags: &[(&str, usize)],
) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let to_map = |pairs: &[(&str, usize)]| -> BTreeMap<String, usize> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    };
    (to_map(kinds), to_map(tags))
}

#[test]
fn drill_down_kind_dominates() {
    // 12/20 = 60.0% kind share, exactly at the threshold so the
    // hint must fire.
    let (kinds, tags) = drill_down_inputs(
        &[("decision", 12), ("fact", 5), ("lesson", 3)],
        &[("rust", 8), ("sqlite", 7), ("mcp", 5)],
    );
    let hint = compute_drill_down_hint(&kinds, &tags, 20).expect("dominant kind fires");
    assert_eq!(hint.facet, "kinds");
    assert_eq!(hint.value, "decision");
    assert_eq!(hint.count, 12);
    assert_eq!(hint.total, 20);
}

#[test]
fn drill_down_tag_dominates() {
    // No kind clears the threshold (max 8/20 = 40%); a tag does
    // (14/20 = 70%) so the function falls through to the tag pass
    // and returns the tag-keyed hint.
    let (kinds, tags) = drill_down_inputs(
        &[("decision", 8), ("fact", 7), ("lesson", 5)],
        &[("session-log", 14), ("rust", 4), ("sqlite", 2)],
    );
    let hint = compute_drill_down_hint(&kinds, &tags, 20).expect("dominant tag fires");
    assert_eq!(hint.facet, "tags");
    assert_eq!(hint.value, "session-log");
    assert_eq!(hint.count, 14);
    assert_eq!(hint.total, 20);
}

#[test]
fn drill_down_below_threshold_none() {
    // Top kind 5/20 = 25%, top tag 7/20 = 35%; neither clears the
    // 60% bar so the function returns None instead of guessing.
    let (kinds, tags) = drill_down_inputs(
        &[("decision", 5), ("fact", 5), ("lesson", 5), ("note", 5)],
        &[("rust", 7), ("sqlite", 7), ("mcp", 6)],
    );
    assert!(compute_drill_down_hint(&kinds, &tags, 20).is_none());
}

#[test]
fn drill_down_single_row_none() {
    // Single-row result sets are trivially "100% dominant" by any
    // facet, but a `narrow:` advisory there has nothing to narrow
    // into. The `total < 2` guard ensures the function bails out.
    let (kinds, tags) = drill_down_inputs(&[("decision", 1)], &[("rust", 1)]);
    assert!(compute_drill_down_hint(&kinds, &tags, 1).is_none());
    // Zero rows also returns None (the same guard catches it).
    let empty: BTreeMap<String, usize> = BTreeMap::new();
    assert!(compute_drill_down_hint(&empty, &empty, 0).is_none());
}

#[test]
fn drill_down_kind_beats_tag_on_tie() {
    // Both facets sit at exactly 60% (6/10 each). Kinds are
    // checked first and a qualifying kind wins outright, so the
    // returned hint is keyed on kinds even though the tag share
    // is identical.
    let (kinds, tags) = drill_down_inputs(
        &[("decision", 6), ("fact", 4)],
        &[("session-log", 6), ("rust", 4)],
    );
    let hint = compute_drill_down_hint(&kinds, &tags, 10).expect("kind tie wins");
    assert_eq!(hint.facet, "kinds");
    assert_eq!(hint.value, "decision");
    assert_eq!(hint.count, 6);
}
