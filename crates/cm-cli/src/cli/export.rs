//! `cm export` — JSON snapshot of the store on stdout.
//!
//! Pure read handler that streams raw JSON to stdout so
//! `cm export > backup.json` works as a Unix pipeline. The capability
//! [`cm_capabilities::export::export`] returns a typed [`ExportView`]
//! which is serialized via [`serde_json::to_string_pretty`] for human
//! readability while remaining valid JSON for `serde_json::from_str`
//! round-trips.
//!
//! No ANSI colours are written to stdout. The handler does not consult
//! [`super::colors`] at all because (a) the output is machine-consumable,
//! and (b) the colours module's TTY-detection guards would already
//! collapse them on `cm export > file.json` anyway. Keeping the handler
//! colour-free makes the contract impossible to violate.
//!
//! Mirrors the MCP `cx_export` handler in
//! `crates/cm-cli/src/mcp/tools/export.rs` so the two channels emit
//! byte-identical snapshots for the same request.

use anyhow::{Context, Result, anyhow};
use cm_capabilities::export::{ExportRequest, export};
use cm_core::ContextStore;

/// `cm export` handler. Read path: no [`WriteContext`] needed.
///
/// Field list mirrors the inline `Commands::Export` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
pub async fn run(
    store: &impl ContextStore,
    scope_path: Option<String>,
    format: Option<String>,
) -> Result<()> {
    let view = export(
        store,
        ExportRequest {
            scope_path,
            format: format.unwrap_or_else(|| "json".to_owned()),
        },
    )
    .await
    .map_err(|e| anyhow!("{e}"))?;

    let json =
        serde_json::to_string_pretty(&view).context("serializing export snapshot to JSON")?;

    // Single `println!` so the trailing newline is exactly one byte. The
    // `serde_json::to_string_pretty` output does not include a trailing
    // newline; adding one makes the file POSIX-compliant when redirected.
    println!("{json}");

    Ok(())
}
