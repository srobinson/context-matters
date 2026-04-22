use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::stats::StatsResult;

/// One `{tag, count}` pair in a [`WebStatsView::top_tags`] list.
///
/// Mirrors `cm_core::TagCount` one-to-one but lives on the projection
/// layer so the web surface does not leak the storage type across the
/// ts-rs boundary.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebStatsTagCount {
    pub tag: String,
    pub count: u64,
}

/// One node in a [`WebStatsView::scope_tree`] list.
///
/// Mirrors `crate::stats::ScopeTreeNode` for the same
/// storage-layer-isolation reason as [`WebStatsTagCount`]. The tree is
/// a flat list (sorted breadth-first lexicographically) so callers can
/// render it without recursing; the `path` field is the full scope
/// path, so structural reconstruction is deterministic from path alone.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebStatsScopeNode {
    pub path: String,
    pub kind: String,
    pub label: String,
    pub entry_count: u64,
}

/// Full projection of a `cx_stats` response for the cm-web HTTP API
/// and the MCP 2025-06-18 `structuredContent` channel.
///
/// Mirrors the YAML `format_stats_view` counters, kind histogram, top
/// tags, and scope tree. `db_size_bytes` is the raw integer for
/// machine consumers; the YAML renderer humanises the same number to
/// a `"4.2 MB"` string, which is intentional: the text channel is
/// for humans, the structured channel is for type-checked clients that
/// want to arithmetic on the value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebStatsView {
    pub active: u64,
    pub superseded: u64,
    pub scopes: u64,
    pub relations: u64,
    pub db_size_bytes: u64,
    pub kinds: BTreeMap<String, u64>,
    pub top_tags: Vec<WebStatsTagCount>,
    pub scope_tree: Vec<WebStatsScopeNode>,
}

/// Maximum tags surfaced in `top_tags`.
///
/// Kept in lock-step with `stats_view::TOP_TAGS_LIMIT`. `stats::stats()`
/// already sorts `entries_by_tag` by the requested order (name or
/// count) before building `StatsResult`, so this projection just takes
/// the prefix; it does not re-sort.
const WEB_STATS_TOP_TAGS_LIMIT: usize = 10;

/// Project a [`StatsResult`] into a [`WebStatsView`].
///
/// Pure transformation; no I/O and no recomputation. All the raw
/// aggregates it needs (`entries_by_kind`, `entries_by_tag`,
/// `scope_tree`) are already built by `stats::stats()` from the
/// storage layer, so this factory just maps field-for-field into the
/// ts-rs-derivable shape. The field-rename story is:
///
/// - `active_entries` -> `active`
/// - `superseded_entries` -> `superseded`
/// - `entries_by_kind` (`HashMap<String, u64>`) -> `kinds` (`BTreeMap`
///   for stable serialisation order)
/// - `entries_by_tag` (`Vec<TagCount>`) -> `top_tags` (bounded to 10)
/// - `scope_tree` (`Vec<ScopeTreeNode>`) -> `scope_tree`
///   (`Vec<WebStatsScopeNode>`, field-renamed one-to-one)
pub fn project_web_stats(result: &StatsResult) -> WebStatsView {
    let kinds: BTreeMap<String, u64> = result
        .stats
        .entries_by_kind
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();

    let top_tags: Vec<WebStatsTagCount> = result
        .stats
        .entries_by_tag
        .iter()
        .take(WEB_STATS_TOP_TAGS_LIMIT)
        .map(|t| WebStatsTagCount {
            tag: t.tag.clone(),
            count: t.count,
        })
        .collect();

    let scope_tree: Vec<WebStatsScopeNode> = result
        .scope_tree
        .iter()
        .map(|node| WebStatsScopeNode {
            path: node.path.clone(),
            kind: node.kind.clone(),
            label: node.label.clone(),
            entry_count: node.entry_count,
        })
        .collect();

    WebStatsView {
        active: result.stats.active_entries,
        superseded: result.stats.superseded_entries,
        scopes: result.stats.scopes,
        relations: result.stats.relations,
        db_size_bytes: result.stats.db_size_bytes,
        kinds,
        top_tags,
        scope_tree,
    }
}
