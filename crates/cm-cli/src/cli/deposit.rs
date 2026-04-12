//! `cm deposit` — batch-store conversation exchanges.
//!
//! Thin CLI handler: reads the `--exchanges` JSON blob (optionally from
//! stdin when `-` is passed), builds a [`DepositRequest`], calls
//! [`cm_capabilities::deposit::deposit`] with a `WriteContext` tagged
//! [`MutationSource::Cli`], and renders the result through the shared
//! [`format_deposit_ack`]. Mirrors the MCP `cx_deposit` handler in
//! `crates/cm-cli/src/mcp/tools/deposit.rs` so the two channels stay
//! byte-identical for the same batch.

use std::io::Read;

use anyhow::{Context, Result, anyhow};
use cm_capabilities::deposit::{self, DepositRequest, Exchange};
use cm_capabilities::projection::format_deposit_ack;
use cm_core::{ContextStore, MutationSource, WriteContext};
use uuid::Uuid;

use crate::cli::scope::resolve_scope;

/// Default attribution stamped on CLI-created entries when `--created-by`
/// is omitted. Distinct from the MCP default (`agent:claude-code`) so
/// operators scanning the store can tell which channel wrote which
/// entry. Changing this affects the `created_by` field of every future
/// deposit that does not override it.
const DEFAULT_CREATED_BY: &str = "agent:cli";

/// `cm deposit` handler. Write path: constructs a [`WriteContext`] with
/// [`MutationSource::Cli`] provenance before delegating to the capability.
///
/// Field list mirrors the inline `Commands::Deposit` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
pub async fn run(
    store: &impl ContextStore,
    exchanges: String,
    summary: Option<String>,
    scope_path: Option<String>,
    created_by: Option<String>,
    json: bool,
) -> Result<()> {
    // `--exchanges -` reads the JSON blob from stdin, matching the
    // `update --body -` convention elsewhere in this CLI. Lets callers
    // pipe long session logs (`cat session.json | cm deposit --exchanges -`)
    // without shell-quoting every newline or argv byte cap.
    let exchanges_json = if exchanges == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read exchanges from stdin")?;
        buf
    } else {
        exchanges
    };

    let exchanges: Vec<Exchange> = serde_json::from_str(&exchanges_json)
        .context("--exchanges must be a JSON array of {user, assistant, title?}")?;

    let scope_path = resolve_scope(scope_path.as_deref());

    let request = DepositRequest {
        exchanges,
        summary,
        scope_path,
        created_by: created_by.unwrap_or_else(|| DEFAULT_CREATED_BY.to_owned()),
    };

    let ctx = WriteContext::new(MutationSource::Cli);

    let result = deposit::deposit(store, request, &ctx)
        .await
        .map_err(|e| anyhow!("{e}"))?;

    if json {
        // No `project_web_deposit` exists — no sibling handler emits a
        // structured JSON payload for writes, and the YAML ack is the
        // canonical wire shape. Mirror that shape here with full
        // hyphenated UUIDs so programmatic callers can feed them back
        // into `cm get` without string massaging.
        let entry_ids: Vec<String> = result.entry_ids.iter().map(Uuid::to_string).collect();
        let summary_id = result.summary_id.map(|id| id.to_string());
        let view = serde_json::json!({
            "entry_ids": entry_ids,
            "summary_id": summary_id,
            "scope_path": result.scope_path,
        });
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        let id_strings: Vec<String> = result.entry_ids.iter().map(Uuid::to_string).collect();
        let summary_str = result.summary_id.map(|id| id.to_string());
        // `format_deposit_ack` already ends with a newline — use `print!`.
        print!(
            "{}",
            format_deposit_ack(&id_strings, summary_str.as_deref(), &result.scope_path)
        );
    }

    Ok(())
}
