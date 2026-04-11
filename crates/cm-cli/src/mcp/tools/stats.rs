//! Handler for the `cx_stats` tool.

use cm_capabilities::projection::format_stats_view;
use cm_capabilities::stats::{self, StatsRequest, TagSort};
use cm_core::ContextStore;
use serde_json::Value;

use crate::mcp::{cm_err_to_string, yaml_response};

pub async fn cx_stats(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let tag_sort_str = args
        .get("tag_sort")
        .and_then(Value::as_str)
        .unwrap_or("name");

    let tag_sort = match tag_sort_str {
        "name" => TagSort::Name,
        "count" => TagSort::Count,
        other => {
            return Err(format!(
                "Validation error: tag_sort must be 'name' or 'count', got '{other}'"
            ));
        }
    };

    let result = stats::stats(store, StatsRequest { tag_sort })
        .await
        .map_err(cm_err_to_string)?;

    yaml_response(format_stats_view(&result))
}
