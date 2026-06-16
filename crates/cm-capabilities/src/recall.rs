//! Recall capability orchestration.

use std::cmp::Ordering;
use std::time::Instant;

use cm_core::{
    CmError, ContextStore, FtsQuery, RecallRankingMode, RecallShadowRecord, ScopePath,
    recall_rank_key,
};

use crate::constants::MAX_LIMIT;
use crate::projection::{RecallRow, entry_has_any_tag, estimate_tokens};
use crate::scope::{ScopeSelector, resolve_scope_selection};
use crate::telemetry::RetrievalLog;

mod metrics;
mod routing;
mod types;

pub use types::{
    DEFAULT_RECALL_SCOPE, RECALL_SCOPE_DEFAULT_ADVISORY, RecallAdvisory, RecallRequest,
    RecallResult, RecallRouting, SearchTier,
};

use routing::route_query;

const RECALL_OVERSAMPLE: u32 = 3;

/// Execute a recall operation against the store.
///
/// Routes to the appropriate query path based on input parameters,
/// applies post-filtering, token budget tracking, and scope chain extraction.
pub async fn recall(
    store: &impl ContextStore,
    request: RecallRequest,
) -> Result<RecallResult, CmError> {
    let mut log = RetrievalLog::from_recall_request(&request);
    let result = recall_inner(store, request, &mut log).await;
    log.emit_recall(&result);
    result
}

async fn recall_inner(
    store: &impl ContextStore,
    request: RecallRequest,
    log: &mut RetrievalLog,
) -> Result<RecallResult, CmError> {
    let scope_defaulted = request.scope.is_none();
    let scope_selector = request
        .scope
        .clone()
        .unwrap_or_else(|| ScopeSelector::Path(ScopePath::global()));
    reject_non_singular_scope(&scope_selector)?;
    let resolved_scope = resolve_scope_selection(store, &scope_selector).await?;
    log.set_resolved_scope(resolved_scope.scope_path.as_ref());
    let scope_path = resolved_scope.scope_path.as_ref();
    let ranking_mode = store.recall_ranking_mode();

    let legacy_fetch_limit = legacy_fetch_limit(&request);
    let fetch_limit = fetch_limit(ranking_mode, &request, legacy_fetch_limit);

    let (raw_rows, routing, actual_fetch_limit, tier) =
        route_query(store, &request, scope_path, fetch_limit).await?;

    let selection = match ranking_mode {
        RecallRankingMode::Legacy => {
            let candidates_before_filter = raw_rows.len();
            let rows = filter_rows(raw_rows, &request);
            RecallSelection {
                rows: apply_token_budget(rank_legacy_rows(rows, &request), request.max_tokens),
                candidates_before_filter,
                fetch_limit_used: actual_fetch_limit,
            }
        }
        RecallRankingMode::Shadow | RecallRankingMode::Live => {
            let started_at = Instant::now();
            let actual_count_before_filter = raw_rows.len();
            let legacy_raw_count =
                legacy_raw_count(&request, actual_count_before_filter, legacy_fetch_limit);
            let legacy_raw_rows = raw_rows
                .iter()
                .take(legacy_raw_count)
                .cloned()
                .collect::<Vec<_>>();
            let full_rows = filter_rows(raw_rows, &request);
            let legacy_rows = filter_rows(legacy_raw_rows, &request);
            select_shadow_rows(
                store,
                ShadowInput {
                    full_rows,
                    legacy_rows,
                    request: &request,
                    routing: &routing,
                    tier,
                    scope_path,
                    ranking_mode,
                    actual_fetch_limit,
                    legacy_fetch_limit_used: legacy_fetch_limit_used(&request, legacy_fetch_limit),
                    candidate_count_before_filter: legacy_raw_count,
                    actual_count_before_filter,
                    started_at,
                },
            )
            .await
        }
    };
    let (budget_rows, total_tokens) = selection.rows;
    let (scope_chain, scope_hits) = scope_chain_and_hits(scope_path, &budget_rows);

    let relation_count_ids = budget_rows.iter().map(|r| r.entry.id).collect::<Vec<_>>();
    let relation_counts = store.count_relations_for(&relation_count_ids).await?;

    Ok(RecallResult {
        entries: budget_rows,
        scope_chain,
        scope_hits,
        token_estimate: total_tokens,
        routing,
        tier,
        candidates_before_filter: selection.candidates_before_filter,
        fetch_limit_used: selection.fetch_limit_used,
        relation_counts,
        advisories: scope_defaulted
            .then(|| RecallAdvisory::ScopeDefaulted {
                applied: DEFAULT_RECALL_SCOPE.to_owned(),
            })
            .into_iter()
            .collect(),
    })
}

struct RecallSelection {
    rows: (Vec<RecallRow>, u32),
    candidates_before_filter: usize,
    fetch_limit_used: u32,
}

struct ShadowInput<'a> {
    full_rows: Vec<RecallRow>,
    legacy_rows: Vec<RecallRow>,
    request: &'a RecallRequest,
    routing: &'a RecallRouting,
    tier: Option<SearchTier>,
    scope_path: Option<&'a ScopePath>,
    ranking_mode: RecallRankingMode,
    actual_fetch_limit: u32,
    legacy_fetch_limit_used: u32,
    candidate_count_before_filter: usize,
    actual_count_before_filter: usize,
    started_at: Instant,
}

async fn select_shadow_rows(store: &impl ContextStore, input: ShadowInput<'_>) -> RecallSelection {
    let ShadowInput {
        full_rows,
        legacy_rows,
        request,
        routing,
        tier,
        scope_path,
        ranking_mode,
        actual_fetch_limit,
        legacy_fetch_limit_used,
        candidate_count_before_filter,
        actual_count_before_filter,
        started_at,
    } = input;
    let candidate_count = full_rows.len();
    let legacy_ranked = rank_legacy_rows(legacy_rows, request);
    let priority_ranked = rank_priority_rows(full_rows, request);
    let window_truncated = window_truncated(&legacy_ranked, &priority_ranked, request.limit);
    let legacy_budget = apply_token_budget(legacy_ranked, request.max_tokens);
    let priority_budget = apply_token_budget(priority_ranked, request.max_tokens);

    let shadow = shadow_record(
        ShadowRecordInput {
            request,
            routing,
            tier,
            scope_path,
            started_at,
        },
        &legacy_budget.0,
        &priority_budget.0,
        candidate_count,
        window_truncated,
    );
    if let Err(error) = store.log_recall_shadow(shadow).await {
        tracing::warn!(?error, "failed to write recall shadow canary row");
    }

    let served_rows = match ranking_mode {
        RecallRankingMode::Shadow => legacy_budget,
        RecallRankingMode::Live => priority_budget,
        RecallRankingMode::Legacy => unreachable!("legacy mode does not compute shadow rows"),
    };
    let fetch_limit_used = match ranking_mode {
        RecallRankingMode::Shadow => legacy_fetch_limit_used,
        RecallRankingMode::Live => actual_fetch_limit,
        RecallRankingMode::Legacy => unreachable!("legacy mode does not compute shadow rows"),
    };

    RecallSelection {
        rows: served_rows,
        candidates_before_filter: if ranking_mode == RecallRankingMode::Shadow {
            candidate_count_before_filter
        } else {
            actual_count_before_filter
        },
        fetch_limit_used,
    }
}

struct ShadowRecordInput<'a> {
    request: &'a RecallRequest,
    routing: &'a RecallRouting,
    tier: Option<SearchTier>,
    scope_path: Option<&'a ScopePath>,
    started_at: Instant,
}

fn shadow_record(
    input: ShadowRecordInput<'_>,
    legacy_rows: &[RecallRow],
    priority_rows: &[RecallRow],
    candidate_count: usize,
    window_truncated: bool,
) -> RecallShadowRecord {
    let metrics = metrics::diff_metrics(legacy_rows, priority_rows, input.request.limit);
    let old_ids = metrics::row_ids(legacy_rows);
    let new_ids = metrics::row_ids(priority_rows);

    RecallShadowRecord {
        scope_path: input.scope_path.map(|scope| scope.as_str().to_owned()),
        query_hash: query_hash(input.request.query.as_deref()),
        query_len: sanitized_query_len(input.request.query.as_deref()),
        routing: routing_name(input.routing).to_owned(),
        tier: input.tier.map(|tier| tier_name(tier).to_owned()),
        k: input.request.limit,
        candidate_count: u32::try_from(candidate_count).unwrap_or(u32::MAX),
        top1_changed: metrics.top1_changed,
        topk_overlap: metrics.topk_overlap,
        footrule: metrics.footrule,
        mean_abs_position_delta: metrics.mean_abs_position_delta,
        position_deltas: metrics.position_deltas,
        old_ids,
        new_ids,
        window_truncated,
        ranking_version: metrics::RANKING_VERSION.to_owned(),
        duration_ms: u32::try_from(input.started_at.elapsed().as_millis()).unwrap_or(u32::MAX),
    }
}

fn query_hash(query: Option<&str>) -> Option<String> {
    query
        .map(sanitized_query)
        .map(|query| blake3::hash(query.as_bytes()).to_hex().to_string())
}

fn sanitized_query_len(query: Option<&str>) -> Option<u32> {
    query
        .map(sanitized_query)
        .map(|query| u32::try_from(query.len()).unwrap_or(u32::MAX))
}

fn sanitized_query(query: &str) -> String {
    FtsQuery::new(query).as_str().to_owned()
}

fn window_truncated(legacy_rows: &[RecallRow], priority_rows: &[RecallRow], limit: u32) -> bool {
    let k = limit as usize;
    priority_rows.iter().take(k).any(|priority_row| {
        legacy_rows
            .iter()
            .position(|legacy_row| legacy_row.entry.id == priority_row.entry.id)
            .is_none_or(|position| position >= k)
    })
}

fn routing_name(routing: &RecallRouting) -> &'static str {
    match routing {
        RecallRouting::Search => "search",
        RecallRouting::TagScopeWalk => "tag_scope_walk",
        RecallRouting::ScopeResolve => "scope_resolve",
        RecallRouting::BrowseFallback => "browse_fallback",
    }
}

fn tier_name(tier: SearchTier) -> &'static str {
    match tier {
        SearchTier::Exact => "exact",
        SearchTier::Prefix => "prefix",
        SearchTier::SplitOr => "split_or",
        SearchTier::None => "none",
    }
}

fn legacy_fetch_limit(request: &RecallRequest) -> u32 {
    if request.query.is_none() && !request.tags.is_empty() {
        request.limit
    } else if has_post_filter(request) {
        oversampled_limit(request.limit)
    } else {
        request.limit
    }
}

fn fetch_limit(
    ranking_mode: RecallRankingMode,
    request: &RecallRequest,
    legacy_fetch_limit: u32,
) -> u32 {
    if matches!(
        ranking_mode,
        RecallRankingMode::Shadow | RecallRankingMode::Live
    ) {
        oversampled_limit(request.limit)
    } else {
        legacy_fetch_limit
    }
}

fn legacy_raw_count(
    request: &RecallRequest,
    actual_count: usize,
    legacy_fetch_limit: u32,
) -> usize {
    if request.query.is_none() && !request.tags.is_empty() {
        actual_count.min(request.limit as usize)
    } else {
        actual_count.min(legacy_fetch_limit as usize)
    }
}

fn legacy_fetch_limit_used(request: &RecallRequest, legacy_fetch_limit: u32) -> u32 {
    if request.query.is_none() && !request.tags.is_empty() {
        MAX_LIMIT
    } else {
        legacy_fetch_limit
    }
}

fn has_post_filter(request: &RecallRequest) -> bool {
    !request.kinds.is_empty() || !request.tags.is_empty()
}

fn oversampled_limit(limit: u32) -> u32 {
    limit.saturating_mul(RECALL_OVERSAMPLE).min(MAX_LIMIT)
}

fn reject_non_singular_scope(selector: &ScopeSelector) -> Result<(), CmError> {
    match selector {
        ScopeSelector::Path(_) | ScopeSelector::CwdInferred { .. } => Ok(()),
        ScopeSelector::Subtree(_) | ScopeSelector::Set(_) | ScopeSelector::All => {
            Err(CmError::InvalidOperationInput {
                op: "cx_recall",
                reason: "scope must resolve to one path; use cx_search for descendants, set, or all scope queries"
                    .to_owned(),
            })
        }
    }
}

fn filter_rows(mut rows: Vec<RecallRow>, request: &RecallRequest) -> Vec<RecallRow> {
    if !request.kinds.is_empty() {
        rows.retain(|row| request.kinds.contains(&row.entry.kind));
    }

    if !request.tags.is_empty() {
        rows.retain(|row| entry_has_any_tag(&row.entry, &request.tags));
    }

    rows
}

fn rank_legacy_rows(mut rows: Vec<RecallRow>, request: &RecallRequest) -> Vec<RecallRow> {
    rows.sort_by_key(|row| std::cmp::Reverse(row.entry.scope_path.depth()));
    rows.truncate(request.limit as usize);
    rows
}

fn rank_priority_rows(mut rows: Vec<RecallRow>, request: &RecallRequest) -> Vec<RecallRow> {
    rows.sort_by(compare_priority_rows);
    rows.truncate(request.limit as usize);
    rows
}

fn compare_priority_rows(left: &RecallRow, right: &RecallRow) -> Ordering {
    recall_rank_key(&left.entry)
        .cmp(&recall_rank_key(&right.entry))
        .then_with(|| compare_bm25(left.score, right.score))
        .then_with(|| right.entry.updated_at.cmp(&left.entry.updated_at))
        .then_with(|| right.entry.id.cmp(&left.entry.id))
}

fn compare_bm25(left: Option<f32>, right: Option<f32>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn apply_token_budget(rows: Vec<RecallRow>, max_tokens: Option<u32>) -> (Vec<RecallRow>, u32) {
    let mut budget_rows = Vec::with_capacity(rows.len());
    let mut total_tokens = 0;

    for row in rows {
        let entry_tokens = estimate_tokens(&row.entry.body);

        if let Some(budget) = max_tokens
            && total_tokens + entry_tokens > budget
            && !budget_rows.is_empty()
        {
            break;
        }

        total_tokens += entry_tokens;
        budget_rows.push(row);
    }

    (budget_rows, total_tokens)
}

fn scope_chain_and_hits(
    scope: Option<&ScopePath>,
    rows: &[RecallRow],
) -> (Vec<String>, Vec<(String, usize)>) {
    match scope {
        Some(scope_path) => {
            let chain: Vec<String> = scope_path.ancestors().map(String::from).collect();
            let hits: Vec<(String, usize)> = chain
                .iter()
                .map(|scope| {
                    let count = rows
                        .iter()
                        .filter(|row| row.entry.scope_path.as_str() == scope)
                        .count();
                    (scope.clone(), count)
                })
                .collect();
            (chain, hits)
        }
        None => {
            let mut seen = std::collections::BTreeMap::<String, usize>::new();
            for row in rows {
                *seen
                    .entry(row.entry.scope_path.as_str().to_owned())
                    .or_default() += 1;
            }
            let mut hits: Vec<(String, usize)> = seen.into_iter().collect();
            hits.sort_by(|a, b| {
                let depth_a = a.0.matches('/').count();
                let depth_b = b.0.matches('/').count();
                depth_b.cmp(&depth_a).then_with(|| a.0.cmp(&b.0))
            });
            let chain: Vec<String> = hits.iter().map(|(scope, _)| scope.clone()).collect();
            (chain, hits)
        }
    }
}
