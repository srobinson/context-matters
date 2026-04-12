//! `cm browse` — paginated inventory of context entries.
//!
//! Thin CLI handler: marshals clap args into a [`BrowseRequest`], calls
//! [`cm_capabilities::browse::browse`], then prints either the text view via
//! [`format_browse_view`] or a pretty-printed JSON projection via
//! [`project_web_browse`]. Mirrors the MCP `cx_browse` handler in
//! `crates/cm-cli/src/mcp/tools/browse.rs` so the two channels stay
//! byte-identical for the same query.
//!
//! Browse semantics differ from recall: omitting `--scope` means *no filter*
//! (return entries from every scope), not "default to global". The handler
//! routes through [`crate::cli::scope::resolve_scope_filter`] which returns
//! `None` on omission and prints the filter-flavor advisory.

use anyhow::{Result, anyhow};
use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{format_browse_view, project_web_browse};
use cm_capabilities::validation::clamp_limit;
use cm_core::{ContextStore, EntryKind, ScopePath};

use crate::cli::scope::resolve_scope_filter;

/// `cm browse` handler. Read-only: no `WriteContext` constructed.
///
/// Field list mirrors the inline `Commands::Browse` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    store: &impl ContextStore,
    scope_path: Option<String>,
    kind: Option<String>,
    tag: Option<String>,
    created_by: Option<String>,
    include_superseded: bool,
    limit: Option<u32>,
    cursor: Option<String>,
    json: bool,
) -> Result<()> {
    let scope_path = match resolve_scope_filter(scope_path.as_deref()) {
        Some(s) => Some(ScopePath::parse(&s).map_err(|e| anyhow!("{e}"))?),
        None => None,
    };

    let kind = match kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(|e| anyhow!("{e}"))?),
        None => None,
    };

    let request = BrowseRequest {
        scope_path,
        kind,
        tag,
        created_by,
        include_superseded,
        limit: clamp_limit(limit),
        cursor,
        ..Default::default()
    };

    let result = browse::browse(store, request.clone())
        .await
        .map_err(|e| anyhow!("{e}"))?;

    if json {
        let view = project_web_browse(&result);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_browse_view` already ends with a newline — use `print!`.
        print!("{}", format_browse_view(&result, &request));
    }

    Ok(())
}
