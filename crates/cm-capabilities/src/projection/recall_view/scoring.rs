/// Min-max normalise raw BM25 scores into `[0.0, 1.0]` with inversion so
/// that a higher normalised value corresponds to a better match.
///
/// `cm-store` surfaces raw SQLite `bm25()` output on `Search`-routed
/// recall rows: floating-point values <= 0 where lower (more negative)
/// means a better match. This function applies
///
/// ```text
///     norm = 1.0 - (raw - min) / (max - min)
/// ```
///
/// so the best (most-negative) raw becomes `1.00` and the worst becomes
/// `0.00`. When every raw score is equal (including the single-row case)
/// the formula's divisor is zero; this function emits `1.00` for every
/// row in that case rather than returning NaN.
pub fn normalise_bm25(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let min = scores.iter().copied().fold(f32::INFINITY, f32::min);
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;
    if range.abs() < f32::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores
        .iter()
        .map(|&raw| 1.0 - (raw - min) / range)
        .collect()
}
