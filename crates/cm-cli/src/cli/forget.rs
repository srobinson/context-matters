//! `cm forget` — soft-delete entries by ID.
//!
//! Thin CLI handler: packs positional IDs into a [`ForgetRequest`], calls
//! [`cm_capabilities::forget::forget`] with a `WriteContext` tagged
//! [`MutationSource::Cli`], then prints the shared
//! [`format_forget_ack`] YAML text. Mirrors the MCP `cx_forget` handler
//! in `crates/cm-cli/src/mcp/tools/forget.rs` so the two channels stay
//! byte-identical for the same batch.

use anyhow::Result;
use cm_capabilities::forget::{self, ForgetRequest};
use cm_capabilities::projection::format_forget_ack;
use cm_core::{ContextStore, MutationSource, WriteContext};

use crate::cli::errors::capability_error;

/// `cm forget` handler. Write path: constructs a [`WriteContext`] with
/// [`MutationSource::Cli`] provenance before delegating to the capability.
///
/// Field list mirrors the inline `Commands::Forget` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
pub async fn run(store: &impl ContextStore, ids: Vec<String>) -> Result<()> {
    let ctx = WriteContext::new(MutationSource::Cli);

    let result = forget::forget(store, ForgetRequest { ids }, &ctx)
        .await
        .map_err(capability_error)?;

    // `format_forget_ack` already ends with a newline — use `print!`.
    print!(
        "{}",
        format_forget_ack(
            result.forgotten,
            result.already_inactive,
            result.not_found,
            &result.errors,
        )
    );

    Ok(())
}
