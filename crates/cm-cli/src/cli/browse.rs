//! `cm browse` — paginated inventory of context entries.
//!
//! Thin CLI handler: marshals clap args into a [`BrowseRequest`], calls
//! [`cm_capabilities::browse::browse`], then prints either the text view via
//! [`format_browse_view`] or a pretty-printed JSON projection via
//! [`project_web_browse`]. Mirrors the MCP `cx_browse` handler in
//! `crates/cm-cli/src/mcp/tools/browse.rs` so the two channels stay
//! byte-identical for the same query.
//!
//! Browse scope defaults are owned by `cm_capabilities::browse`. When the
//! capability applies a default, this adapter only renders the returned
//! advisory to stderr.

use anyhow::Result;
use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{format_browse_view, project_web_browse};
use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::validation::parse_kind;
use cm_core::ContextStore;

use crate::cli::errors::{capability_error, string_error};
use crate::cli::scope::print_advisory;

/// `cm browse` handler. Read-only: no `WriteContext` constructed.
///
/// Field list mirrors the inline `Commands::Browse` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    store: &impl ContextStore,
    scope: Option<String>,
    cwd: Option<String>,
    include_resolution: bool,
    kind: Option<String>,
    tag: Option<String>,
    created_by: Option<String>,
    include_superseded: bool,
    limit: Option<u32>,
    cursor: Option<String>,
    json: bool,
) -> Result<()> {
    let cwd = match cwd {
        Some(raw) if raw.trim().is_empty() => {
            return Err(string_error("Invalid parameters: cwd cannot be empty"));
        }
        Some(raw) => Some(raw.into()),
        None => None,
    };
    let scope =
        ScopeSelector::from_optional_scope(scope.as_deref(), cwd).map_err(capability_error)?;

    let kind = match kind {
        Some(k) => Some(parse_kind(&k).map_err(string_error)?),
        None => None,
    };

    let request = BrowseRequest {
        scope,
        include_resolution: include_resolution.then_some(true),
        kind,
        tag,
        created_by,
        include_superseded,
        limit,
        cursor,
        ..Default::default()
    };

    let result = browse::browse(store, request.clone())
        .await
        .map_err(capability_error)?;

    if let Some(advisory) = result.advisory.as_deref() {
        print_advisory(advisory);
    }

    if json {
        let view = project_web_browse(&result);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_browse_view` already ends with a newline — use `print!`.
        print!("{}", format_browse_view(&result, &request));
    }

    Ok(())
}
