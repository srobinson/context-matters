//! `ContentSearchPage` formatter and serialisable view.
//!
//! Shared by the MCP `cx_search` tool and the CLI `cm search` command so
//! both surfaces render the same search result shape.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::Utc;
use cm_core::{ContentSearchPage, ScoredEntry};
use serde::{Deserialize, Serialize};

use super::{
    HighlightStyle, SNIPPET_MAX_BYTES, collapse_whitespace, estimate_tokens, kind_histogram,
    relative_age, render_histogram, scope_histogram, smart_snippet, tag_histogram,
};
use crate::projection::normalise_bm25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchView {
    pub header: SearchHeader,
    pub entries: Vec<SearchRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHeader {
    pub query: String,
    pub returned: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub scope_hits: BTreeMap<String, usize>,
    pub kinds_histogram: BTreeMap<String, u32>,
    pub tags_histogram: BTreeMap<String, u32>,
    pub tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRow {
    pub id: String,
    pub score: f32,
    pub title: String,
    pub snippet: String,
    pub age: String,
    pub scope: String,
    pub kind: String,
    pub tags: Vec<String>,
}

pub fn project_search_view(query: &str, page: ContentSearchPage) -> SearchView {
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
            SearchRow {
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

    SearchView {
        header: SearchHeader {
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

pub fn format_search_view(view: &SearchView) -> String {
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
        let _ = writeln!(out, "# more: pass cursor `{cursor}` to fetch the next page");
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
