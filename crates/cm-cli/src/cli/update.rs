//! `cm update` — partial update an entry by ID.
//!
//! Thin CLI handler: parses optional flags, reads body from stdin when
//! `--body -` is passed, projects `--meta` JSON through the shared
//! [`MetaInput`] into an [`EntryMeta`], constructs an [`UpdateEntry`], then
//! calls [`ContextStore::update_entry`] with a [`WriteContext`] tagged
//! [`MutationSource::Cli`]. Mirrors the MCP `cx_update` handler in
//! `crates/cm-cli/src/mcp/tools/update.rs`.
//!
//! Note: `update` does NOT route through a `cm_capabilities::update::update`
//! function (no such function exists). It calls [`ContextStore::update_entry`]
//! directly, exactly as the MCP handler does. The "capability layer" for
//! `update` is [`format_update_ack`] on the text branch and the shared
//! [`MetaInput`] projection on the input side.

use std::io::Read;

use anyhow::{Context, Result, anyhow, bail};
use cm_capabilities::projection::format_update_ack;
use cm_capabilities::validation::{MetaInput, parse_kind};
use cm_core::{ContextStore, MutationSource, UpdateEntry, WriteContext};
use uuid::Uuid;

/// `cm update` handler. Write path: constructs a [`WriteContext`] with
/// [`MutationSource::Cli`] provenance before calling
/// [`ContextStore::update_entry`].
///
/// Field list mirrors the inline `Commands::Update` clap variant in
/// [`super::cli_def`]. The destructure happens at the call site in
/// `main.rs`; this keeps the handler decoupled from the clap surface.
pub async fn run(
    store: &impl ContextStore,
    id: String,
    title: Option<String>,
    body: Option<String>,
    kind: Option<String>,
    meta: Option<String>,
    json: bool,
) -> Result<()> {
    let uuid = Uuid::parse_str(&id).map_err(|e| anyhow!("invalid UUID '{id}': {e}"))?;

    // `--body -` reads from stdin, matching the fmm CLI convention. Lets
    // callers pipe multi-line markdown edits without shell-quoting every
    // newline.
    let body = match body {
        Some(s) if s == "-" => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("failed to read body from stdin")?;
            Some(buf)
        }
        other => other,
    };

    // `--meta` takes a JSON object matching the wire shape of `cx_update`'s
    // `meta` argument. Parse failures surface with the serde_json column
    // pointer so the caller sees exactly where the blob went wrong.
    let meta = match meta {
        Some(raw) => Some(
            serde_json::from_str::<MetaInput>(&raw)
                .with_context(|| "--meta must be a valid JSON object".to_owned())?
                .into_entry_meta()
                .map_err(|e| anyhow!("{e}"))?,
        ),
        None => None,
    };

    let kind = match kind {
        Some(k) => Some(parse_kind(&k).map_err(|e| anyhow!("{e}"))?),
        None => None,
    };

    if title.is_none() && body.is_none() && kind.is_none() && meta.is_none() {
        bail!("at least one field must be provided (--title, --body, --kind, --meta)");
    }

    let update = UpdateEntry {
        title,
        body,
        kind,
        meta,
    };

    let ctx = WriteContext::new(MutationSource::Cli);

    let entry = store
        .update_entry(uuid, update, &ctx)
        .await
        .map_err(|e| anyhow!("{e}"))?;

    if json {
        // No `project_web_update` exists — no sibling handler emits a
        // FullEntryView-shaped JSON payload for writes, and the YAML ack is
        // the canonical wire shape. Mirror that shape here with the full
        // 64-char content_hash so programmatic callers can compare against
        // the cm-store value directly rather than the 8-char display
        // prefix used on the text branch.
        let view = serde_json::json!({
            "updated": entry.id.to_string(),
            "content_hash": entry.content_hash,
        });
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_update_ack` already ends with a newline — use `print!`.
        print!(
            "{}",
            format_update_ack(&entry.id.to_string(), &entry.content_hash)
        );
    }

    Ok(())
}
