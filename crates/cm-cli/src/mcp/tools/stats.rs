//! Handler for the `cx_stats` tool.

use cm_capabilities::projection::{format_stats_view, project_web_stats};
use cm_capabilities::stats::{self, StatsRequest};
use cm_capabilities::validation::parse_tag_sort;
use cm_core::ContextStore;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, reject_removed_scope_inputs, reject_unknown_fields,
};

pub async fn cx_stats(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    reject_removed_scope_inputs(args)?;
    reject_unknown_fields(args, &["tag_sort"])?;

    let tag_sort_str = args
        .get("tag_sort")
        .and_then(Value::as_str)
        .unwrap_or("name");

    let tag_sort = parse_tag_sort(tag_sort_str)?;

    let result = stats::stats(store, StatsRequest { tag_sort })
        .await
        .map_err(cm_err_to_string)?;

    let text = format_stats_view(&result);
    let view = project_web_stats(&result);
    dual_response(text, &view)
}
