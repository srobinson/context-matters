//! Handler for the `cx_stats` tool.

use cm_capabilities::stats::{self, StatsRequest, TagSort};
use cm_core::ContextStore;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

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

    let scope_tree: Vec<Value> = result
        .scope_tree
        .iter()
        .map(|n| {
            json!({
                "path": n.path,
                "kind": n.kind,
                "label": n.label,
                "entry_count": n.entry_count,
            })
        })
        .collect();

    let tags_json: Vec<Value> = result
        .stats
        .entries_by_tag
        .iter()
        .map(|tc| json!({"tag": tc.tag, "count": tc.count}))
        .collect();

    let response = json!({
        "active_entries": result.stats.active_entries,
        "superseded_entries": result.stats.superseded_entries,
        "scopes": result.stats.scopes,
        "relations": result.stats.relations,
        "entries_by_kind": result.stats.entries_by_kind,
        "entries_by_scope": result.stats.entries_by_scope,
        "entries_by_tag": tags_json,
        "db_size_bytes": result.stats.db_size_bytes,
        "scope_tree": scope_tree,
    });

    json_response(response)
}
