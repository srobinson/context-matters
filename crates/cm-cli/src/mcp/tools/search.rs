//! Handler for the `cx_search` tool.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::Utc;
use cm_capabilities::projection::{
    HighlightStyle, SNIPPET_MAX_BYTES, collapse_whitespace, estimate_tokens, kind_histogram,
    normalise_bm25, relative_age, render_histogram, scope_histogram, smart_snippet, tag_histogram,
};
use cm_capabilities::scope::{ScopeSelector, resolve_scope_filter};
use cm_capabilities::search;
use cm_capabilities::validation::{check_input_size, clamp_limit, parse_kind};
use cm_core::{ContentSearchPage, ContentSearchRequest, ContextStore, EntryKind, ScoredEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, dual_response, parse_params};
use crate::shared::reject_removed_scope_inputs;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CxSearchParams {
    query: String,
    scope: ScopeSelector,

    #[serde(default)]
    kinds: Vec<String>,

    #[serde(default)]
    tags: Vec<String>,

    #[serde(default)]
    limit: Option<u32>,

    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Serialize)]
struct CxSearchView {
    header: CxSearchHeader,
    entries: Vec<CxSearchRow>,
}

#[derive(Debug, Serialize)]
struct CxSearchHeader {
    query: String,
    returned: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    scope_hits: BTreeMap<String, usize>,
    kinds_histogram: BTreeMap<String, u32>,
    tags_histogram: BTreeMap<String, u32>,
    tokens: u32,
}

#[derive(Debug, Serialize)]
struct CxSearchRow {
    id: String,
    score: f32,
    title: String,
    snippet: String,
    age: String,
    scope: String,
    kind: String,
    tags: Vec<String>,
}

pub async fn cx_search(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    let params: CxSearchParams = parse_params(args)?;
    check_input_size(&params.query, "query")?;

    let kinds: Vec<EntryKind> = params
        .kinds
        .iter()
        .map(|k| parse_kind(k))
        .collect::<Result<Vec<_>, _>>()?;
    let kinds = (!kinds.is_empty()).then_some(kinds);
    let tags = (!params.tags.is_empty()).then_some(params.tags);
    let limit = clamp_limit(params.limit);
    let scope = resolve_scope_filter(store, &params.scope)
        .await
        .map_err(cm_err_to_string)?;

    let request = ContentSearchRequest {
        query: params.query,
        scope,
        kinds,
        tags,
        limit,
        cursor: params.cursor,
    };

    let page = search::search(store, request.clone())
        .await
        .map_err(cm_err_to_string)?;
    let view = project_search_view(&request.query, page);
    let text = format_search_view(&view);
    dual_response(text, &view)
}

fn project_search_view(query: &str, page: ContentSearchPage) -> CxSearchView {
    let now = Utc::now();
    let scores = page.items.iter().map(|item| item.score).collect::<Vec<_>>();
    let norm_scores = normalise_bm25(&scores);
    let scope_hits = scope_histogram(&page.items, |item| item.entry.scope_path.as_str());
    let kinds_histogram =
        histogram_to_u32(kind_histogram(&page.items, |item| item.entry.kind.as_str()));
    let tags_histogram = histogram_to_u32(tag_histogram(&page.items, entry_tags));
    let tokens = page
        .items
        .iter()
        .map(|item| estimate_tokens(&item.entry.body))
        .sum();

    let entries = page
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let raw_snippet = smart_snippet(
                &item.entry.body,
                Some(query),
                HighlightStyle::Bracketed,
                SNIPPET_MAX_BYTES,
            );
            CxSearchRow {
                id: item.entry.id.to_string(),
                score: norm_scores.get(idx).copied().unwrap_or(0.0),
                title: item.entry.title.clone(),
                snippet: collapse_whitespace(&raw_snippet),
                age: relative_age(item.entry.updated_at, now),
                scope: item.entry.scope_path.as_str().to_owned(),
                kind: item.entry.kind.as_str().to_owned(),
                tags: entry_tags(item).to_vec(),
            }
        })
        .collect::<Vec<_>>();

    CxSearchView {
        header: CxSearchHeader {
            query: query.to_owned(),
            returned: entries.len(),
            next_cursor: page.next_cursor,
            scope_hits,
            kinds_histogram,
            tags_histogram,
            tokens,
        },
        entries,
    }
}

fn format_search_view(view: &CxSearchView) -> String {
    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "---");
    let _ = writeln!(out, "query: {}", view.header.query);
    let _ = writeln!(out, "returned: {}", view.header.returned);
    if let Some(cursor) = &view.header.next_cursor {
        let _ = writeln!(out, "next_cursor: {cursor}");
    }
    if !view.header.scope_hits.is_empty() {
        let _ = writeln!(
            out,
            "scope_hits: {}",
            render_histogram(&view.header.scope_hits)
        );
    }
    render_u32_histogram(&mut out, "kinds", &view.header.kinds_histogram);
    render_u32_histogram(&mut out, "tags", &view.header.tags_histogram);
    let _ = writeln!(out, "tokens: {}", view.header.tokens);
    out.push_str("\nentries:\n");
    if view.entries.is_empty() {
        out.push_str("  []\n");
        return out;
    }
    for row in &view.entries {
        let _ = writeln!(out, "  - {:.2}  {}", row.score, row.title);
        if !row.snippet.is_empty() {
            let _ = writeln!(out, "        {}", row.snippet);
        }
        let tags = if row.tags.is_empty() {
            String::new()
        } else {
            format!("  tags: {}", row.tags.join(", "))
        };
        let _ = writeln!(
            out,
            "        # scope: {}  kind: {}{}  age: {}",
            row.scope, row.kind, tags, row.age
        );
    }
    if let Some(cursor) = &view.header.next_cursor {
        let _ = writeln!(out, "# more - cx_search(cursor=\"{cursor}\") to page");
    }
    out
}

fn entry_tags(item: &ScoredEntry) -> &[String] {
    item.entry
        .meta
        .as_ref()
        .map(|meta| meta.tags.as_slice())
        .unwrap_or(&[])
}

fn histogram_to_u32(src: BTreeMap<String, usize>) -> BTreeMap<String, u32> {
    src.into_iter().map(|(k, v)| (k, v as u32)).collect()
}

fn render_u32_histogram(out: &mut String, label: &str, hist: &BTreeMap<String, u32>) {
    if hist.is_empty() {
        return;
    }
    let rendered = hist
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "{label}: {rendered}");
}
