//! Handler for the `cx_forget` tool.

use cm_capabilities::forget::{self, ForgetRequest};
use cm_capabilities::projection::format_forget_ack;
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

    let text = format_forget_ack(
        result.forgotten,
        result.already_inactive,
        result.not_found,
        &result.errors,
    );
    let errors: Vec<_> = result
        .errors
        .iter()
        .map(|error| serde_json::json!({"id": error.id, "error": error.error}))
        .collect();
    let structured = serde_json::json!({
        "forgotten": result.forgotten,
        "already_inactive": result.already_inactive,
        "not_found": result.not_found,
        "errors": errors
    });
    dual_response(text, &structured)
}
