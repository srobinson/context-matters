use crate::recall::{RecallRouting, SearchTier};

/// Header rendering for the `routing:` line: `(tag, one-line explanation)`.
///
/// The tag matches the serde `rename_all = "snake_case"` rendering of the
/// enum so callers searching by routing name find the same string in
/// the text envelope and the structured log channel.
///
/// Crate-visible so [`crate::projection::web_view`] can pick the same
/// tag for `WebRecallHeader::routing` without re-matching every enum
/// variant. Only the `.0` tag is needed there; the explanation text is
/// YAML-specific and stays in the trailer.
pub(crate) fn routing_explanation(routing: &RecallRouting) -> (&'static str, &'static str) {
    match routing {
        RecallRouting::Search => ("search", "FTS5 ranking"),
        RecallRouting::TagScopeWalk => ("tag_scope_walk", "tag + ancestor walk"),
        RecallRouting::ScopeResolve => ("scope_resolve", "recent entries in scope"),
        RecallRouting::BrowseFallback => ("browse_fallback", "recency fallback"),
    }
}

/// Trailer rendering for the `# routing: ...` advisory line: `(tag, next-step hint)`.
///
/// Re-uses the `routing_explanation` tag so the header and trailer agree
/// on the canonical snake_case name. The advisory tells the caller how
/// to broaden or narrow the query if the current result set is unhelpful.
pub(super) fn routing_advice(routing: &RecallRouting) -> (&'static str, &'static str) {
    let tag = routing_explanation(routing).0;
    let advice = match routing {
        RecallRouting::Search => {
            "re-query with OR between synonyms or prefix match (term*) for more breadth"
        }
        RecallRouting::TagScopeWalk => {
            "no FTS query supplied; try a free-text query, broader tag, or higher scope"
        }
        RecallRouting::ScopeResolve => {
            "returning most-recent entries in scope; add a query or tag to narrow"
        }
        RecallRouting::BrowseFallback => {
            "no FTS match in scope; falling back to most-recent entries anywhere"
        }
    };
    (tag, advice)
}

/// Header suffix tag for the cascade's winning [`SearchTier`]. Returns
/// the snake_case name for the three winning tiers and `None` for
/// [`SearchTier::None`], so the header stays clean when all three
/// tiers were exhausted (the empty-result trailer covers that case).
///
/// Crate-visible so [`crate::projection::web_view`] can project the
/// same tag into `WebRecallHeader::tier`. Shared so the YAML and web
/// views cannot drift on the stringified tier name.
pub(crate) fn search_tier_header_tag(tier: SearchTier) -> Option<&'static str> {
    match tier {
        SearchTier::Exact => Some("exact"),
        SearchTier::Prefix => Some("prefix"),
        SearchTier::SplitOr => Some("split_or"),
        SearchTier::None => None,
    }
}

/// Trailer advisory line for the cascade's winning [`SearchTier`].
/// Fires only on `Prefix` and `SplitOr`: those tiers produced a result
/// set from a query shape the caller did not write, so the LLM needs
/// to be told about the rewrite to learn what succeeded. `Exact` is
/// the happy path (silent); `None` is covered by the `no matches`
/// trailer, so a tier advisory there would be redundant noise.
pub(super) fn search_tier_trailer(tier: SearchTier) -> Option<String> {
    let (tag, advice) = match tier {
        SearchTier::Prefix => (
            "prefix",
            "original query had zero exact hits, tried prefix match",
        ),
        SearchTier::SplitOr => (
            "split_or",
            "original query had zero prefix hits, OR-joined tokens",
        ),
        SearchTier::Exact | SearchTier::None => return None,
    };
    Some(format!("# tier: {tag} - {advice}"))
}
