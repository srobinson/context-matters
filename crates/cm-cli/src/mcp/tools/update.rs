//! Handler for the `cx_update` tool.

use cm_capabilities::projection::format_update_ack;
use cm_capabilities::validation::{MetaInput, parse_kind};
use cm_core::{ContextStore, MutationSource, UpdateEntry, WriteContext};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, check_input_size, cm_err_to_string, parse_params, yaml_response};

#[derive(Debug, Deserialize)]
struct CxUpdateParams {
    /// ID of the entry to update.
    id: String,

    /// New title.
    #[serde(default)]
    title: Option<String>,

    /// New body content.
    #[serde(default)]
    body: Option<String>,

    /// New kind classification.
    #[serde(default)]
    kind: Option<String>,

    /// Replace metadata entirely.
    #[serde(default)]
    meta: Option<MetaInput>,
}

pub async fn cx_update(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let params: CxUpdateParams = parse_params(args)?;

    let id = uuid::Uuid::parse_str(&params.id)
        .map_err(|_| format!("Invalid UUID format: '{}'", params.id))?;

    // Validate at least one field is provided
    if params.title.is_none()
        && params.body.is_none()
        && params.kind.is_none()
        && params.meta.is_none()
    {
        return Err("Validation error: at least one field must be provided".to_owned());
    }

    // Validate input sizes
    if let Some(ref t) = params.title {
        check_input_size(t, "title")?;
    }
    if let Some(ref b) = params.body {
        check_input_size(b, "body")?;
    }

    // Parse kind if provided
    let kind = match &params.kind {
        Some(k) => Some(parse_kind(k)?),
        None => None,
    };

    // Project the shared `MetaInput` wire shape into an `EntryMeta`. The
    // CLI `cm update --meta` handler uses the same projection for
    // byte-identical semantics across channels.
    let meta = match params.meta {
        Some(m) => Some(m.into_entry_meta()?),
        None => None,
    };

    let update = UpdateEntry {
        title: params.title,
        body: params.body,
        kind,
        meta,
    };

    let ctx = WriteContext::new(MutationSource::Mcp);

    let entry = store
        .update_entry(id, update, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    yaml_response(format_update_ack(
        &entry.id.to_string(),
        &entry.content_hash,
    ))
}
