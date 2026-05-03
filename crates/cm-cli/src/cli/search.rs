//! `cm search` — content search across explicit scopes.
//!
//! Thin CLI handler: parses clap args into a [`ContentSearchRequest`],
//! calls [`cm_capabilities::search::search`], then prints the shared search
//! view used by MCP `cx_search`.

use anyhow::Result;
use cm_capabilities::projection::{format_search_view, project_search_view};
use cm_capabilities::scope::{ScopeSelector, resolve_scope_filter};
use cm_capabilities::search;
use cm_capabilities::validation::{check_input_size, clamp_limit, parse_kind};
use cm_core::{ContentSearchRequest, ContextStore, EntryKind};

use crate::cli::errors::{capability_error, string_error};
use crate::shared::normalize_scope_selector_input;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    store: &impl ContextStore,
    query: String,
    scope: String,
    kinds: Vec<String>,
    tags: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
    json: bool,
) -> Result<()> {
    check_input_size(&query, "query").map_err(string_error)?;

    let scope = normalize_scope_selector_input(&scope);
    let selector = ScopeSelector::parse(&scope).map_err(capability_error)?;
    let scope = resolve_scope_filter(store, &selector)
        .await
        .map_err(capability_error)?;

    let kinds: Vec<EntryKind> = kinds
        .iter()
        .map(|k| parse_kind(k).map_err(string_error))
        .collect::<Result<Vec<_>, _>>()?;

    let request = ContentSearchRequest {
        query,
        scope,
        kinds: (!kinds.is_empty()).then_some(kinds),
        tags: (!tags.is_empty()).then_some(tags),
        limit: clamp_limit(limit),
        cursor,
    };

    let page = search::search(store, request.clone())
        .await
        .map_err(capability_error)?;
    let view = project_search_view(&request.query, page);
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        print!("{}", format_search_view(&view));
    }
    Ok(())
}
