//! Handler for the `cx_update` tool.

use cm_capabilities::projection::format_update_ack;
use cm_capabilities::update::{self, UpdateRequest};
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, parse_params, yaml_response};

pub async fn cx_update(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let request: UpdateRequest = parse_params(args)?;
    let ctx = WriteContext::new(MutationSource::Mcp);

    let result = update::update(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    yaml_response(format_update_ack(&result.updated_id, &result.content_hash))
}
