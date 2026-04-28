//! Handler for the `cx_get` tool.

use cm_capabilities::get::{self, GetRequest};
use cm_capabilities::projection::{format_get_view, project_web_get};
use cm_core::ContextStore;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
};

pub async fn cx_get(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    let request: GetRequest = parse_params(args)?;
    let result = get::get(store, request).await.map_err(cm_err_to_string)?;

    let text = format_get_view(&result.entries, &result.requested_ids);
    let view = project_web_get(&result.entries, &result.requested_ids);
    dual_response(text, &view)
}
