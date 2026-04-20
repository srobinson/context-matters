//! `cm get` — fetch full entry content by ID.
//!
//! Two-phase retrieval consumer: `cm recall` / `cm browse` surface IDs, and
//! `cm get` pulls full bodies. Mirrors the MCP `cx_get` handler in
//! `crates/cm-cli/src/mcp/tools/get.rs` so the two channels stay
//! byte-identical for the same query.
//!
//! Note: `get` does NOT route through a `cm_capabilities::get::get` function
//! (no such function exists). It calls `ContextStore::get_entries` directly,
//! exactly as the MCP handler does. The "capability layer" for `get` is the
//! formatter pair (`format_get_view` + `project_web_get`).

use anyhow::{Result, bail};
use cm_capabilities::constants::MAX_BATCH_IDS;
use cm_capabilities::projection::{format_get_view, project_web_get};
use cm_core::ContextStore;
use uuid::Uuid;

/// `cm get` handler. Read-only: no `WriteContext` constructed.
///
/// Field list mirrors the inline `Commands::Get` clap variant in
/// [`super::cli_def`].
pub async fn run(store: &impl ContextStore, ids: Vec<String>, json: bool) -> Result<()> {
    if ids.is_empty() {
        bail!("ids cannot be empty (pass at least one entry id)");
    }
    if ids.len() > MAX_BATCH_IDS {
        bail!("maximum {MAX_BATCH_IDS} ids per request");
    }

    // Each input must be a full hyphenated UUIDv7. Anything that fails
    // `Uuid::parse_str` errors the whole batch so malformed input surfaces
    // crisply instead of silently missing rows.
    //
    // `canonical_ids` runs in lock-step with `uuids`: it carries the string
    // form that the projection layer's missing-set diff compares against
    // `Entry::id.to_string()`. Normalizing to `Uuid::to_string()` lets the
    // caller type the UUID in any accepted format (uppercase, no hyphens)
    // and still match the formatter's canonical render.
    let mut uuids: Vec<Uuid> = Vec::with_capacity(ids.len());
    let mut canonical_ids: Vec<String> = Vec::with_capacity(ids.len());
    for raw in &ids {
        let id = Uuid::parse_str(raw).map_err(|e| anyhow::anyhow!("invalid UUID '{raw}': {e}"))?;
        uuids.push(id);
        canonical_ids.push(id.to_string());
    }

    let entries = store
        .get_entries(&uuids)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if json {
        let view = project_web_get(&entries, &canonical_ids);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_get_view` already ends with a newline — use `print!`.
        print!("{}", format_get_view(&entries, &canonical_ids));
    }

    Ok(())
}
