//! Tool list schema for MCP `tools/list` response.
//!
//! In the final build, this is generated from `tools.toml` by `build.rs`.
//! For now, provides a handwritten tool list matching the 9 `cx_*` tools.

use serde_json::{Value, json};

/// Return the MCP `tools/list` response payload.
pub(super) fn tool_list() -> Value {
    json!({
        "tools": [
            tool_cx_recall(),
            tool_cx_store(),
            tool_cx_deposit(),
            tool_cx_browse(),
            tool_cx_get(),
            tool_cx_update(),
            tool_cx_forget(),
            tool_cx_stats(),
            tool_cx_export(),
        ]
    })
}

fn tool_cx_recall() -> Value {
    json!({
        "name": "cx_recall",
        "description": "Search and retrieve context entries. Primary retrieval tool. Call at session start with a summary of the task to load relevant knowledge.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "FTS5 search query."},
                "scope": {"type": "string", "description": "Scope path. Default: 'global'."},
                "kinds": {"type": "array", "items": {"type": "string"}, "description": "Filter by entry kinds."},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Filter by tags."},
                "limit": {"type": "integer", "description": "Max entries. Default: 20, max: 200."},
                "max_tokens": {"type": "integer", "description": "Token budget for response."}
            }
        }
    })
}

fn tool_cx_store() -> Value {
    json!({
        "name": "cx_store",
        "description": "Store a context entry with structured metadata. Auto-creates scope chain if needed.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "body": {"type": "string"},
                "kind": {"type": "string"},
                "scope_path": {"type": "string"},
                "created_by": {"type": "string"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "confidence": {"type": "string"},
                "source": {"type": "string"},
                "expires_at": {"type": "string"},
                "priority": {"type": "integer"},
                "supersedes": {"type": "string"}
            },
            "required": ["title", "body", "kind"]
        }
    })
}

fn tool_cx_deposit() -> Value {
    json!({
        "name": "cx_deposit",
        "description": "Batch-deposit conversation exchanges for future context.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "exchanges": {"type": "array", "items": {"type": "object", "properties": {"user": {"type": "string"}, "assistant": {"type": "string"}}, "required": ["user", "assistant"]}},
                "summary": {"type": "string"},
                "scope_path": {"type": "string"},
                "created_by": {"type": "string"}
            },
            "required": ["exchanges"]
        }
    })
}

fn tool_cx_browse() -> Value {
    json!({
        "name": "cx_browse",
        "description": "Browse entries with filters and cursor pagination.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "scope_path": {"type": "string"},
                "kind": {"type": "string"},
                "tag": {"type": "string"},
                "created_by": {"type": "string"},
                "include_superseded": {"type": "boolean"},
                "limit": {"type": "integer"},
                "cursor": {"type": "string"}
            }
        }
    })
}

fn tool_cx_get() -> Value {
    json!({
        "name": "cx_get",
        "description": "Fetch full content for specific entry IDs.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["ids"]
        }
    })
}

fn tool_cx_update() -> Value {
    json!({
        "name": "cx_update",
        "description": "Partially update an existing entry.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "string"},
                "title": {"type": "string"},
                "body": {"type": "string"},
                "kind": {"type": "string"},
                "meta": {"type": "object"}
            },
            "required": ["id"]
        }
    })
}

fn tool_cx_forget() -> Value {
    json!({
        "name": "cx_forget",
        "description": "Soft-delete entries by marking them as forgotten.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["ids"]
        }
    })
}

fn tool_cx_stats() -> Value {
    json!({
        "name": "cx_stats",
        "description": "View store statistics and scope breakdown.",
        "inputSchema": {"type": "object", "properties": {}}
    })
}

fn tool_cx_export() -> Value {
    json!({
        "name": "cx_export",
        "description": "Export entries and scopes as JSON.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "scope_path": {"type": "string"},
                "format": {"type": "string"}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_has_nine_tools() {
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 9);
    }

    #[test]
    fn all_tools_have_cx_prefix() {
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            assert!(name.starts_with("cx_"), "Tool '{name}' missing cx_ prefix");
        }
    }

    #[test]
    fn tool_names_match_expected() {
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec![
                "cx_recall",
                "cx_store",
                "cx_deposit",
                "cx_browse",
                "cx_get",
                "cx_update",
                "cx_forget",
                "cx_stats",
                "cx_export"
            ]
        );
    }
}
