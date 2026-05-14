//! Handler for the `cx_forget` tool.

use cm_capabilities::forget::{self, ForgetRequest};
use cm_capabilities::projection::{format_forget_ack, project_forget_receipt};
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
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

    let receipt = project_forget_receipt(&result);
    let text = format_forget_ack(
        receipt.forgotten,
        receipt.already_inactive,
        receipt.not_found,
        &result.errors,
    );
    dual_response(text, &receipt)
}
