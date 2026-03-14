//! Handler for the `cx_stats` tool.

use cm_core::ContextStore;
use cm_store::CmStore;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

pub fn cx_stats(store: &CmStore, _args: &Value) -> Result<String, String> {
    let stats = store.stats().map_err(cm_err_to_string)?;
    let scopes = store.list_scopes(None).map_err(cm_err_to_string)?;

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

    let response = json!({
        "active_entries": stats.active_entries,
        "superseded_entries": stats.superseded_entries,
        "scopes": stats.scopes,
        "relations": stats.relations,
        "entries_by_kind": stats.entries_by_kind,
        "entries_by_scope": stats.entries_by_scope,
        "db_size_bytes": stats.db_size_bytes,
        "scope_tree": scope_tree,
    });

    json_response(response)
}
