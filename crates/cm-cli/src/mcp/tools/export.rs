//! Handler for the `cx_export` tool.
//!
//! Thin adapter that parses MCP params, delegates to
//! [`cm_capabilities::export::export`], and serializes the resulting
//! [`cm_capabilities::export::ExportView`] back into a JSON-RPC response.
//! All format validation, scope filtering, and snapshot assembly live in
//! the capability so the CLI handler in `crates/cm-cli/src/cli/export.rs`
//! emits a byte-identical shape for the same request.

use cm_capabilities::export::{ExportRequest, export};
use cm_core::ContextStore;
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, json_response, parse_params, reject_removed_scope_inputs,
};
use crate::shared::parse_structured_scope_selector;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CxExportParams {
    /// Filter to a scope selector.
    #[serde(default)]
    scope: Option<Value>,

    /// Export format. Only "json" supported.
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "json".to_owned()
}

pub async fn cx_export(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    let params: CxExportParams = parse_params(args)?;
    let scope = parse_structured_scope_selector(params.scope)?;

    let view = export(
        store,
        ExportRequest {
            scope,
            format: params.format,
        },
    )
    .await
    .map_err(cm_err_to_string)?;

    let response =
        serde_json::to_value(&view).map_err(|e| format!("serializing export view: {e}"))?;

    json_response(response)
}
