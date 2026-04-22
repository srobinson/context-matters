//! Handler for the `cx_store` tool.

use cm_capabilities::projection::format_store_ack;
use cm_capabilities::store::{StoreRequest, store as store_entry};
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, parse_params, yaml_response};

pub async fn cx_store(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);
    let request: StoreRequest = parse_params(args)?;

    let result = store_entry(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;
    yaml_response(format_store_ack(
        &result.entry_id,
        &result.scope_path,
        result.kind.as_str(),
        &result.content_hash,
        result.superseded_id.as_deref(),
    ))
}
