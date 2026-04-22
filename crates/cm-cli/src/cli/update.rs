//! `cm update` partial update entry by ID.
//!
//! Thin CLI handler: parses optional flags, reads body from stdin when
//! `--body -` is passed, constructs an [`UpdateRequest`], then delegates to
//! `cm_capabilities::update`.

use std::io::Read;

use anyhow::{Context, Result, anyhow};
use cm_capabilities::error::cm_err_to_string;
use cm_capabilities::projection::{format_update_ack, project_web_update};
use cm_capabilities::update::{self, UpdateRequest};
use cm_capabilities::validation::MetaInput;
use cm_core::{ContextStore, MutationSource, WriteContext};

/// `cm update` handler. Write path: constructs a [`WriteContext`] with
/// [`MutationSource::Cli`] provenance before calling the shared capability.
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
                .with_context(|| "--meta must be a valid JSON object".to_owned())?,
        ),
        None => None,
    };

    let request = UpdateRequest {
        id,
        title,
        body,
        kind,
        meta,
    };

    let ctx = WriteContext::new(MutationSource::Cli);

    let result = update::update(store, request, &ctx)
        .await
        .map_err(|e| anyhow!("{}", cm_err_to_string(e)))?;

    if json {
        let view = project_web_update(&result);
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        // `format_update_ack` already ends with a newline — use `print!`.
        print!(
            "{}",
            format_update_ack(&result.updated_id, &result.content_hash)
        );
    }

    Ok(())
}
