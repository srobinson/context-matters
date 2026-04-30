//! Recall capability orchestration.

use cm_core::{CmError, ContextStore, ScopePath};

use crate::constants::MAX_LIMIT;
use crate::projection::{RecallRow, entry_has_any_tag, estimate_tokens};
use crate::scope::{ScopeSelector, resolve_scope_selection};

mod routing;
mod types;

pub use types::{
    DEFAULT_RECALL_SCOPE, RECALL_SCOPE_DEFAULT_ADVISORY, RecallAdvisory, RecallRequest,
    RecallResult, RecallRouting, SearchTier,
};

use routing::route_query;

/// Execute a recall operation against the store.
///
/// Routes to the appropriate query path based on input parameters,
/// applies post-filtering, token budget tracking, and scope chain extraction.
pub async fn recall(
    store: &impl ContextStore,
    request: RecallRequest,
) -> Result<RecallResult, CmError> {
    let scope_defaulted = request.scope.is_none();
    let scope_selector = request
        .scope
        .clone()
        .unwrap_or_else(|| ScopeSelector::Path(ScopePath::global()));
    reject_non_singular_scope(&scope_selector)?;
    let resolved_scope = resolve_scope_selection(store, &scope_selector).await?;
    let scope_path = resolved_scope.scope_path.as_ref();

    let has_post_filter = !request.kinds.is_empty() || !request.tags.is_empty();
    let fetch_limit = if has_post_filter {
        request.limit.saturating_mul(3).min(MAX_LIMIT)
    } else {
        request.limit
    };

    let (raw_rows, routing, actual_fetch_limit, tier) =
        route_query(store, &request, scope_path, fetch_limit).await?;
    let candidates_before_filter = raw_rows.len();

    let rows = post_filter_rows(raw_rows, &request);
    let (budget_rows, total_tokens) = apply_token_budget(rows, request.max_tokens);
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
        candidates_before_filter,
        fetch_limit_used: actual_fetch_limit,
        relation_counts,
        advisories: scope_defaulted
            .then(|| RecallAdvisory::ScopeDefaulted {
                applied: DEFAULT_RECALL_SCOPE.to_owned(),
            })
            .into_iter()
            .collect(),
    })
}

fn reject_non_singular_scope(selector: &ScopeSelector) -> Result<(), CmError> {
    match selector {
        ScopeSelector::Path(_) | ScopeSelector::CwdInferred { .. } => Ok(()),
        ScopeSelector::Subtree(_) | ScopeSelector::Set(_) | ScopeSelector::All => {
            Err(CmError::InvalidOperationInput {
                op: "cx_recall",
                reason: "scope must resolve to one path; use cx_search for subtree, set, or all scope queries"
                    .to_owned(),
            })
        }
    }
}

fn post_filter_rows(mut rows: Vec<RecallRow>, request: &RecallRequest) -> Vec<RecallRow> {
    if !request.kinds.is_empty() {
        rows.retain(|row| request.kinds.contains(&row.entry.kind));
    }

    if !request.tags.is_empty() {
        rows.retain(|row| entry_has_any_tag(&row.entry, &request.tags));
    }

    rows.sort_by_key(|row| std::cmp::Reverse(row.entry.scope_path.depth()));
    rows.truncate(request.limit as usize);
    rows
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
