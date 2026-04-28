//! Handler for the `cx_forget` tool.

use cm_capabilities::forget::{self, ForgetRequest};
use cm_capabilities::projection::format_forget_ack;
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, parse_params, reject_removed_scope_inputs, yaml_response,
};

#[derive(Debug, Deserialize)]
struct CxForgetParams {
    /// Entry IDs to forget. Maximum 100 per request.
    ids: Vec<String>,
}

pub async fn cx_forget(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);
    reject_removed_scope_inputs(args)?;
    let params: CxForgetParams = parse_params(args)?;

    let result = forget::forget(store, ForgetRequest { ids: params.ids }, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    yaml_response(format_forget_ack(
        result.forgotten,
        result.already_inactive,
        result.not_found,
        &result.errors,
    ))
}
