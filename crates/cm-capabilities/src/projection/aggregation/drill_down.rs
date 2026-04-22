use std::collections::BTreeMap;

/// Share of the most-frequent facet at or above which the formatter
/// emits a faceted drill-down advisory. `0.60` means: if one kind or
/// tag accounts for 60% or more of a result set, the renderer tells
/// the caller how to re-query narrowed to that facet.
///
/// Exposed as a named constant so the 60% figure lives in one place
/// and every site that reasons about dominance (the hint builder plus
/// the unit tests) reads the same value.
pub const DRILL_DOWN_THRESHOLD: f32 = 0.60;

/// A single-facet dominance hint derived from a result set's kind and
/// tag histograms. `facet` is either `"kinds"` or `"tags"` and maps
/// directly to the filter argument name on `cx_recall` / `cx_browse`
/// so the rendered advisory can embed it verbatim.
///
/// `count` uses `usize` to match the upstream `BTreeMap<String, usize>`
/// histogram shape without a lossy cast; `total` is the full row count
/// the histogram was computed from. The pair lets the formatter emit
/// `"...: N of M results are X"` without re-counting the slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrillDownHint {
    pub facet: String,
    pub value: String,
    pub count: usize,
    pub total: usize,
}

/// Compute a faceted drill-down hint from the kind and tag histograms.
///
/// Returns `Some` when one facet's most-frequent value accounts for at
/// least [`DRILL_DOWN_THRESHOLD`] of `total`, otherwise `None`. Kinds
/// are checked first and a qualifying kind wins outright without
/// inspecting tags, so kind dominance beats tag dominance on ties
/// (e.g. both at exactly 60%). Single-row result sets (`total < 2`)
/// always return `None` because a trivially-dominant single row would
/// emit a `narrow:` advisory that offers nothing to narrow into.
///
/// The "most-frequent" value within a histogram is selected
/// deterministically: alphabetical first-wins on intra-histogram ties,
/// mirroring how [`super::render_histogram`] surfaces ties in header order.
pub fn compute_drill_down_hint(
    kinds: &BTreeMap<String, usize>,
    tags: &BTreeMap<String, usize>,
    total: usize,
) -> Option<DrillDownHint> {
    if total < 2 {
        return None;
    }
    if let Some(hint) = dominant_facet("kinds", kinds, total) {
        return Some(hint);
    }
    dominant_facet("tags", tags, total)
}

/// Helper for [`compute_drill_down_hint`]. Picks the most-frequent
/// entry from `hist` and emits a [`DrillDownHint`] when that entry's
/// share of `total` is at least [`DRILL_DOWN_THRESHOLD`]. `facet` is
/// the caller-supplied tag (`"kinds"` or `"tags"`) embedded in the
/// hint unchanged.
fn dominant_facet(
    facet: &str,
    hist: &BTreeMap<String, usize>,
    total: usize,
) -> Option<DrillDownHint> {
    let (value, count) = most_frequent(hist)?;
    let share = count as f32 / total as f32;
    if share < DRILL_DOWN_THRESHOLD {
        return None;
    }
    Some(DrillDownHint {
        facet: facet.to_owned(),
        value: value.to_owned(),
        count,
        total,
    })
}

/// Pick the most-frequent `(key, count)` pair from a histogram with a
/// deterministic tie-break: walks the `BTreeMap` in its natural
/// alphabetical order and keeps the first entry at any given max
/// count. Empty histograms return `None`.
fn most_frequent(hist: &BTreeMap<String, usize>) -> Option<(&str, usize)> {
    let mut best: Option<(&str, usize)> = None;
    for (k, &v) in hist {
        match best {
            None => best = Some((k.as_str(), v)),
            Some((_, best_count)) if v > best_count => best = Some((k.as_str(), v)),
            // Strict >: a later entry at the same count is not preferred, so
            // alphabetical-first wins on intra-histogram ties.
            _ => {}
        }
    }
    best
}
