//! `cm recall` — search and retrieve context entries.
//!
//! Thin CLI handler: marshals clap args into a [`RecallRequest`], calls
//! [`cm_capabilities::recall::recall`], then prints either the text view via
//! [`format_recall_view`] or a pretty-printed JSON projection via
//! [`project_web_recall`]. Mirrors the MCP `cx_recall` handler in
//! `crates/cm-cli/src/mcp/tools/recall.rs` so the two channels stay
//! byte-identical for the same query.

use anyhow::{Result, anyhow};
use cm_capabilities::projection::{format_recall_view, project_web_recall};
use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::{check_input_size, clamp_limit, parse_kind};
use cm_core::{ContextStore, EntryKind, ScopePath};

use crate::cli::errors::capability_error;
use crate::cli::scope::print_advisory;

/// `cm recall` handler. Read-only: no `WriteContext` constructed.
///
/// Field list mirrors the inline `Commands::Recall` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    store: &impl ContextStore,
    query: Option<String>,
    scope: Option<String>,
    kinds: Vec<String>,
    tags: Vec<String>,
    limit: Option<u32>,
    max_tokens: Option<u32>,
    json: bool,
) -> Result<()> {
    if let Some(ref q) = query {
        check_input_size(q, "query").map_err(|e| anyhow!("{e}"))?;
    }

    let scope = match scope.as_deref() {
        Some(s) => Some(ScopePath::parse(s).map_err(capability_error)?),
        None => None,
    };

    let kinds: Vec<EntryKind> = kinds
        .iter()
        .map(|k| parse_kind(k).map_err(|e| anyhow!("{e}")))
        .collect::<Result<Vec<_>, _>>()?;

    let request = RecallRequest {
        query,
        scope,
        kinds,
        tags,
        limit: clamp_limit(limit),
        max_tokens,
    };

    // Clone so the projection calls below can still borrow `&request`.
    let result = recall::recall(store, request.clone())
        .await
        .map_err(capability_error)?;

    for advisory in &result.advisories {
        print_advisory(advisory.body());
    }

    if json {
        let view = project_web_recall(&result, &request);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_recall_view` already ends with a newline — use `print!`.
        print!("{}", format_recall_view(&result, &request));
    }

    Ok(())
}
