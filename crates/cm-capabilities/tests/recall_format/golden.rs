use cm_capabilities::projection::format_recall_view_at;
use cm_capabilities::recall::SearchTier;

use super::fixtures::{
    browse_fallback_fixture, dedup_fixture, empty_fixture, rels_fixture, search_fixture,
    search_fixture_with_tier,
};

const GOLDEN_SEARCH: &str = include_str!("../snapshots/recall_view_search.txt");
const GOLDEN_SEARCH_PREFIX_TIER: &str =
    include_str!("../snapshots/recall_view_search_prefix_tier.txt");
const GOLDEN_SEARCH_SPLIT_OR_TIER: &str =
    include_str!("../snapshots/recall_view_search_split_or_tier.txt");
const GOLDEN_BROWSE_FALLBACK: &str = include_str!("../snapshots/recall_view_browse_fallback.txt");
const GOLDEN_EMPTY: &str = include_str!("../snapshots/recall_view_empty.txt");
const GOLDEN_DEDUP: &str = include_str!("../snapshots/recall_view_dedup.txt");
const GOLDEN_RELS: &str = include_str!("../snapshots/recall_view_rels.txt");

#[test]
fn format_recall_view_matches_search_golden() {
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SEARCH,
        "rendered recall search view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_search_prefix_tier_golden() {
    // Prefix tier: header carries `, tier: prefix` and the trailer
    // emits a `# tier: prefix - ...` advisory teaching the caller
    // that the exact implicit AND query returned zero rows and the
    // cascade advanced to the prefix match tier.
    let (result, request, now) = search_fixture_with_tier(SearchTier::Prefix);
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SEARCH_PREFIX_TIER,
        "rendered recall search (prefix tier) view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_search_split_or_tier_golden() {
    // SplitOr tier: header carries `, tier: split_or` and the
    // trailer emits a `# tier: split_or - ...` advisory teaching
    // the caller that both the exact and prefix tiers returned
    // zero rows before the OR joined cascade arm succeeded.
    let (result, request, now) = search_fixture_with_tier(SearchTier::SplitOr);
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_SEARCH_SPLIT_OR_TIER,
        "rendered recall search (split_or tier) view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_browse_fallback_golden() {
    let (result, request, now) = browse_fallback_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_BROWSE_FALLBACK,
        "rendered recall browse_fallback view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_empty_golden() {
    let (result, request, now) = empty_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_EMPTY,
        "rendered recall empty view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn format_recall_view_matches_dedup_golden() {
    // Intra response dedup hint: rows 1 and 3 carry the same
    // `deaddeaddeaddead...` content hash prefix, so row 3 must pick
    // up a `dup_of: 019dedaa` annotation in its trailing YAML
    // comment. Row 2's hash differs entirely, so it renders without
    // annotation. Row 1 is the leader and is also unannotated.
    let (result, request, now) = dedup_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_DEDUP,
        "rendered recall dedup view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
    assert_eq!(
        rendered.matches("dup_of:").count(),
        1,
        "exactly one dup_of annotation expected (row 3 only):\n{rendered}",
    );
}

#[test]
fn format_recall_view_matches_rels_golden() {
    // Relation count annotations: rows 1 and 2 carry populated
    // `relation_counts` entries (3 and 1 respectively), so their
    // trailing YAML comments must pick up `rels: 3` and `rels: 1`.
    // Row 3 is absent from the map and renders without any rels
    // annotation at all.
    let (result, request, now) = rels_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert_eq!(
        rendered, GOLDEN_RELS,
        "rendered recall rels view does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
    assert!(
        rendered.contains("rels: 3"),
        "row 1 should carry rels: 3:\n{rendered}",
    );
    assert!(
        rendered.contains("rels: 1"),
        "row 2 should carry rels: 1:\n{rendered}",
    );
    assert_eq!(
        rendered.matches("rels: ").count(),
        2,
        "exactly two rels annotations expected (rows 1 and 2):\n{rendered}",
    );
    let row3_line = rendered
        .lines()
        .find(|l| l.contains("scope: global  kind: lesson"))
        .expect("row 3 comment line present");
    assert!(
        !row3_line.contains("rels:"),
        "row 3 should carry no rels annotation:\n{row3_line}",
    );
}
