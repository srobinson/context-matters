//! `cm stats` — store statistics via `cm_capabilities::stats`.
//!
//! Thin CLI handler: marshals clap args into a [`StatsRequest`], calls
//! [`cm_capabilities::stats::stats`], then prints either the text view via
//! [`format_stats_view`] or a pretty-printed JSON projection via
//! [`project_web_stats`]. Mirrors the MCP `cx_stats` handler in
//! `crates/cm-cli/src/mcp/tools/stats.rs` so the two channels stay
//! byte-identical for the same query.

use anyhow::{Result, anyhow, bail};
use cm_capabilities::projection::{format_stats_view, project_web_stats};
use cm_capabilities::stats::{self, StatsRequest, TagSort};
use cm_core::ContextStore;

/// `cm stats` handler. Read-only: no `WriteContext` constructed.
///
/// Field list mirrors the inline `Commands::Stats` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
pub async fn run(store: &impl ContextStore, tag_sort: Option<String>, json: bool) -> Result<()> {
    let tag_sort = match tag_sort.as_deref().unwrap_or("name") {
        "name" => TagSort::Name,
        "count" => TagSort::Count,
        other => bail!("tag_sort must be 'name' or 'count', got '{other}'"),
    };

    let result = stats::stats(store, StatsRequest { tag_sort })
        .await
        .map_err(|e| anyhow!("{e}"))?;

    if json {
        let view = project_web_stats(&result);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_stats_view` already ends with a newline — use `print!`.
        print!("{}", format_stats_view(&result));
    }

    Ok(())
}
