//! Handler for the `cx_get` tool.

use cm_capabilities::get::{self, GetRequest};
use cm_capabilities::projection::{format_get_view, project_web_get};
use cm_core::{CmError, ContextStore};
use serde_json::Value;

use crate::mcp::{ToolResult, dual_response, parse_params};

pub async fn cx_get(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let request: GetRequest = parse_params(args)?;
    let result = get::get(store, request).await.map_err(get_err_to_string)?;

    let text = format_get_view(&result.entries, &result.requested_ids);
    let view = project_web_get(&result.entries, &result.requested_ids);
    dual_response(text, &view)
}

fn get_err_to_string(e: CmError) -> String {
    match e {
        CmError::Validation(msg) => format!("Validation error: {msg}"),
        other => cm_capabilities::error::cm_err_to_string(other),
    }
}
