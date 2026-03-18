//! Handler for the `cx_stats` tool.

use cm_core::ContextStore;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

pub async fn cx_stats(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let tag_sort = args
        .get("tag_sort")
        .and_then(Value::as_str)
        .unwrap_or("name");

    if tag_sort != "name" && tag_sort != "count" {
        return Err(format!(
            "Validation error: tag_sort must be 'name' or 'count', got '{tag_sort}'"
        ));
    }

    let stats = store.stats().await.map_err(cm_err_to_string)?;
    let scopes = store.list_scopes(None).await.map_err(cm_err_to_string)?;

    let scope_tree: Vec<Value> = scopes
        .iter()
        .map(|s| {
            let entry_count = stats
                .entries_by_scope
                .get(s.path.as_str())
                .copied()
                .unwrap_or(0);
            json!({
                "path": s.path.as_str(),
                "kind": s.kind.as_str(),
                "label": &s.label,
                "entry_count": entry_count,
            })
        })
        .collect();

    // The store returns tags sorted by count DESC (SQL ORDER BY cnt DESC).
    // Re-sort if the caller wants alphabetical order.
    let mut entries_by_tag = stats.entries_by_tag;
    if tag_sort == "name" {
        entries_by_tag.sort_by(|a, b| a.tag.cmp(&b.tag));
    }

    let tags_json: Vec<Value> = entries_by_tag
        .iter()
        .map(|tc| json!({"tag": tc.tag, "count": tc.count}))
        .collect();

    let response = json!({
        "active_entries": stats.active_entries,
        "superseded_entries": stats.superseded_entries,
        "scopes": stats.scopes,
        "relations": stats.relations,
        "entries_by_kind": stats.entries_by_kind,
        "entries_by_scope": stats.entries_by_scope,
        "entries_by_tag": tags_json,
        "db_size_bytes": stats.db_size_bytes,
        "scope_tree": scope_tree,
    });

    json_response(response)
}
