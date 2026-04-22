use super::super::fmt_with_commas;
use super::routing::{routing_advice, search_tier_trailer};
use super::*;
use crate::recall::{RecallRouting, SearchTier};

#[test]
fn normalise_bm25_inverts_negative_raw_scores() {
    // Raw BM25 values as surfaced by `cm-store` on a Search-routed
    // recall: all negative, lower = better. Expected normalisation
    // after inversion: the most-negative raw maps to 1.00 (best),
    // the least-negative maps to 0.00 (worst).
    //
    // Formula: norm = 1.0 - (raw - min) / (max - min)
    //   min=-3.47, max=-0.88, range=2.59
    //   -3.47 -> 1.0 - 0.00 / 2.59 = 1.00
    //   -1.12 -> 1.0 - 2.35 / 2.59 ≈ 0.09
    //   -0.88 -> 1.0 - 2.59 / 2.59 = 0.00
    //
    // NOTE: ALP-1731's spec example listed the non-inverted values
    // `[0.00, 0.91, 1.00]`, which would map the best match to 0.00.
    // That is a spec-authoring mistake; the formula and the
    // store-side "lower = better" convention require inversion.
    let raws = [-3.47_f32, -1.12, -0.88];
    let norms = normalise_bm25(&raws);
    assert_eq!(round2(norms[0]), 1.00);
    assert_eq!(round2(norms[1]), 0.09);
    assert_eq!(round2(norms[2]), 0.00);
}

#[test]
fn normalise_bm25_uniform_scores_collapse_to_one() {
    // When every raw score is equal, the divisor is zero. The
    // function emits 1.00 for every row rather than returning NaN.
    assert_eq!(normalise_bm25(&[-2.5, -2.5, -2.5]), vec![1.0, 1.0, 1.0]);
    // Single-row slices also hit the uniform branch.
    assert_eq!(normalise_bm25(&[-1.0]), vec![1.0]);
}

#[test]
fn normalise_bm25_empty_is_empty() {
    assert!(normalise_bm25(&[]).is_empty());
}

#[test]
fn routing_explanation_covers_every_variant() {
    assert_eq!(routing_explanation(&RecallRouting::Search).0, "search");
    assert_eq!(
        routing_explanation(&RecallRouting::TagScopeWalk).0,
        "tag_scope_walk",
    );
    assert_eq!(
        routing_explanation(&RecallRouting::ScopeResolve).0,
        "scope_resolve",
    );
    assert_eq!(
        routing_explanation(&RecallRouting::BrowseFallback).0,
        "browse_fallback",
    );
    // Every explanation is non-empty so the header `#` comment
    // never renders as a dangling prefix.
    for routing in [
        RecallRouting::Search,
        RecallRouting::TagScopeWalk,
        RecallRouting::ScopeResolve,
        RecallRouting::BrowseFallback,
    ] {
        let (tag, explain) = routing_explanation(&routing);
        assert!(!tag.is_empty() && !explain.is_empty());
    }
}

#[test]
fn routing_advice_tag_matches_routing_explanation_tag() {
    for routing in [
        RecallRouting::Search,
        RecallRouting::TagScopeWalk,
        RecallRouting::ScopeResolve,
        RecallRouting::BrowseFallback,
    ] {
        assert_eq!(
            routing_explanation(&routing).0,
            routing_advice(&routing).0,
            "routing tag must agree between header and trailer for {routing:?}",
        );
        assert!(!routing_advice(&routing).1.is_empty());
    }
}

#[test]
fn search_tier_header_tag_hides_none_variant() {
    // Exact / Prefix / SplitOr surface in the header; SearchTier::None
    // returns None so the header omits the tier suffix when the
    // cascade exhausted every tier without a hit.
    assert_eq!(search_tier_header_tag(SearchTier::Exact), Some("exact"));
    assert_eq!(search_tier_header_tag(SearchTier::Prefix), Some("prefix"));
    assert_eq!(
        search_tier_header_tag(SearchTier::SplitOr),
        Some("split_or"),
    );
    assert_eq!(search_tier_header_tag(SearchTier::None), None);
}

#[test]
fn search_tier_trailer_fires_only_on_fallback_tiers() {
    // Exact is the happy path (no advisory). None is already covered
    // by the `no matches` trailer, so duplicating it would be noise.
    assert!(search_tier_trailer(SearchTier::Exact).is_none());
    assert!(search_tier_trailer(SearchTier::None).is_none());
    // Prefix and SplitOr emit a trailing advisory describing the
    // rewrite so the LLM learns which fallback succeeded.
    let prefix = search_tier_trailer(SearchTier::Prefix).expect("prefix emits");
    assert!(
        prefix.starts_with("# tier: prefix - "),
        "prefix advisory shape: {prefix}",
    );
    assert!(
        prefix.contains("zero exact hits"),
        "prefix advisory text: {prefix}",
    );
    let split_or = search_tier_trailer(SearchTier::SplitOr).expect("split_or emits");
    assert!(
        split_or.starts_with("# tier: split_or - "),
        "split_or advisory shape: {split_or}",
    );
    assert!(
        split_or.contains("OR-joined"),
        "split_or advisory text: {split_or}",
    );
}

#[test]
fn fmt_with_commas_inserts_thousands_separators() {
    // Canonical behaviour is tested in the aggregation module; this
    // test only guards the recall-side call sites against a regression
    // that would stop accepting `u32` through the `impl Into<u64>`
    // signature when the helper moved out of this file.
    assert_eq!(fmt_with_commas(0_u32), "0");
    assert_eq!(fmt_with_commas(3_420_u32), "3,420");
}

/// Round to two decimal places for assertion-friendly comparisons
/// against the normalised BM25 output.
fn round2(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}
