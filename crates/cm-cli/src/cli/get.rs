//! `cm get` — fetch full entry content by ID.
//!
//! Two-phase retrieval consumer: `cm recall` / `cm browse` surface IDs, and
//! `cm get` pulls full bodies. Validation and store access live in
//! `cm_capabilities::get`; this adapter only constructs the request and
//! renders the shared projection.

use anyhow::Result;
use cm_capabilities::get::{self, GetRequest};
use cm_capabilities::projection::{format_get_view, project_web_get};
use cm_core::ContextStore;

use crate::cli::errors::capability_error;

/// `cm get` handler. Read-only: no `WriteContext` constructed.
///
/// Field list mirrors the inline `Commands::Get` clap variant in
/// [`super::cli_def`].
pub async fn run(store: &impl ContextStore, ids: Vec<String>, json: bool) -> Result<()> {
    let result = get::get(store, GetRequest { ids })
        .await
        .map_err(capability_error)?;

    if json {
        let view = project_web_get(&result.entries, &result.requested_ids);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_get_view` already ends with a newline — use `print!`.
        print!(
            "{}",
            format_get_view(&result.entries, &result.requested_ids)
        );
    }

    Ok(())
}
