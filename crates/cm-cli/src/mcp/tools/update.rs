//! Handler for the `cx_update` tool.

use cm_capabilities::projection::{format_update_ack, project_update_receipt};
use cm_capabilities::update::{self, UpdateRequest};
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
};

pub async fn cx_update(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    let request: UpdateRequest = parse_params(args)?;
    let ctx = WriteContext::new(MutationSource::Mcp);

    let result = update::update(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    let receipt = project_update_receipt(&result);
    let text = format_update_ack(&receipt.id, &receipt.content_hash);
    dual_response(text, &receipt)
}
