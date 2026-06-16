use std::collections::{BTreeSet, HashMap, HashSet};

use cm_core::RecallShadowPositionDelta;
use uuid::Uuid;

use crate::projection::RecallRow;

pub(super) const RANKING_VERSION: &str = "recall-ranking-v1";

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RecallDiffMetrics {
    pub top1_changed: bool,
    pub topk_overlap: f64,
    pub footrule: f64,
    pub mean_abs_position_delta: f64,
    pub position_deltas: Vec<RecallShadowPositionDelta>,
}

pub(super) fn diff_metrics(
    old_rows: &[RecallRow],
    new_rows: &[RecallRow],
    requested_k: u32,
) -> RecallDiffMetrics {
    let old_ids = row_ids(old_rows);
    let new_ids = row_ids(new_rows);
    let k = effective_k(requested_k, old_ids.len(), new_ids.len());

    let top1_changed = old_ids.first() != new_ids.first();
    let topk_overlap = topk_overlap(&old_ids, &new_ids, k);
    let position_deltas = position_deltas(&old_ids, &new_ids, k);
    let footrule = footrule(&old_ids, &new_ids, k);
    let mean_abs_position_delta = mean_abs_position_delta(&position_deltas);

    RecallDiffMetrics {
        top1_changed,
        topk_overlap,
        footrule,
        mean_abs_position_delta,
        position_deltas,
    }
}

pub(super) fn row_ids(rows: &[RecallRow]) -> Vec<Uuid> {
    rows.iter().map(|row| row.entry.id).collect()
}

fn effective_k(requested_k: u32, old_len: usize, new_len: usize) -> usize {
    (requested_k as usize).min(old_len.max(new_len))
}

fn topk_overlap(old_ids: &[Uuid], new_ids: &[Uuid], k: usize) -> f64 {
    if k == 0 {
        return 1.0;
    }

    let old_set: HashSet<Uuid> = old_ids.iter().take(k).copied().collect();
    let overlap = new_ids
        .iter()
        .take(k)
        .filter(|id| old_set.contains(id))
        .count();

    overlap as f64 / k as f64
}

fn footrule(old_ids: &[Uuid], new_ids: &[Uuid], k: usize) -> f64 {
    if k == 0 {
        return 0.0;
    }

    let old_positions = positions(old_ids);
    let new_positions = positions(new_ids);
    let ids = union_ids(old_ids, new_ids);
    let missing_position = k + 1;
    let total: usize = ids
        .iter()
        .map(|id| {
            let old_position = old_positions.get(id).copied().unwrap_or(missing_position);
            let new_position = new_positions.get(id).copied().unwrap_or(missing_position);
            old_position.abs_diff(new_position)
        })
        .sum();

    if total == 0 {
        return 0.0;
    }

    let denominator = footrule_denominator(old_ids, new_ids, k);
    total as f64 / denominator as f64
}

fn footrule_denominator(old_ids: &[Uuid], new_ids: &[Uuid], k: usize) -> usize {
    let old_set: HashSet<Uuid> = old_ids.iter().copied().collect();
    let new_set: HashSet<Uuid> = new_ids.iter().copied().collect();
    let full_permutation = old_ids.len() == k && new_ids.len() == k && old_set == new_set;

    if full_permutation {
        (k * k) / 2
    } else {
        k * (k + 1)
    }
    .max(1)
}

fn mean_abs_position_delta(position_deltas: &[RecallShadowPositionDelta]) -> f64 {
    if position_deltas.is_empty() {
        return 0.0;
    }

    let total: u32 = position_deltas
        .iter()
        .map(|delta| delta.delta.unsigned_abs())
        .sum();
    total as f64 / position_deltas.len() as f64
}

fn position_deltas(old_ids: &[Uuid], new_ids: &[Uuid], k: usize) -> Vec<RecallShadowPositionDelta> {
    let old_positions = positions(old_ids);
    let new_positions = positions(new_ids);
    let missing_position = k + 1;

    union_ids(old_ids, new_ids)
        .into_iter()
        .map(|id| {
            let old_position = old_positions.get(&id).copied();
            let new_position = new_positions.get(&id).copied();
            let old_rank = old_position.unwrap_or(missing_position);
            let new_rank = new_position.unwrap_or(missing_position);
            RecallShadowPositionDelta {
                id,
                old_position: old_position.map(as_u32),
                new_position: new_position.map(as_u32),
                delta: as_i32(new_rank) - as_i32(old_rank),
            }
        })
        .collect()
}

fn positions(ids: &[Uuid]) -> HashMap<Uuid, usize> {
    ids.iter()
        .copied()
        .enumerate()
        .map(|(index, id)| (id, index + 1))
        .collect()
}

fn union_ids(old_ids: &[Uuid], new_ids: &[Uuid]) -> Vec<Uuid> {
    let mut ids = BTreeSet::new();
    ids.extend(old_ids.iter().copied());
    ids.extend(new_ids.iter().copied());
    ids.into_iter().collect()
}

fn as_u32(position: usize) -> u32 {
    u32::try_from(position).unwrap_or(u32::MAX)
}

fn as_i32(position: usize) -> i32 {
    i32::try_from(position).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_pair_has_full_footrule_distance() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        let metrics = diff_ids(&[a, b], &[b, a], 2);

        assert!(metrics.top1_changed);
        assert_eq!(metrics.topk_overlap, 1.0);
        assert_eq!(metrics.footrule, 1.0);
        assert_eq!(metrics.mean_abs_position_delta, 1.0);
    }

    #[test]
    fn disjoint_topk_has_zero_overlap_and_full_footrule_distance() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();
        let d = Uuid::now_v7();

        let metrics = diff_ids(&[a, b], &[c, d], 2);

        assert!(metrics.top1_changed);
        assert_eq!(metrics.topk_overlap, 0.0);
        assert_eq!(metrics.footrule, 1.0);
        assert_eq!(metrics.position_deltas.len(), 4);
    }

    #[test]
    fn underfilled_identical_recall_has_full_overlap() {
        let a = Uuid::now_v7();

        let metrics = diff_ids(&[a], &[a], 10);

        assert!(!metrics.top1_changed);
        assert_eq!(metrics.topk_overlap, 1.0);
        assert_eq!(metrics.footrule, 0.0);
        assert_eq!(metrics.mean_abs_position_delta, 0.0);
        assert_eq!(metrics.position_deltas.len(), 1);
    }

    fn diff_ids(old_ids: &[Uuid], new_ids: &[Uuid], k: u32) -> RecallDiffMetrics {
        let old_rows = rows(old_ids);
        let new_rows = rows(new_ids);
        diff_metrics(&old_rows, &new_rows, k)
    }

    fn rows(ids: &[Uuid]) -> Vec<RecallRow> {
        ids.iter()
            .map(|id| RecallRow {
                entry: cm_core::Entry {
                    id: *id,
                    scope_path: cm_core::ScopePath::global(),
                    kind: cm_core::EntryKind::Fact,
                    title: String::new(),
                    body: String::new(),
                    content_hash: String::new(),
                    meta: None,
                    created_by: String::new(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    superseded_by: None,
                },
                score: None,
            })
            .collect()
    }
}
