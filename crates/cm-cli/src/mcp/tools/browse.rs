//! Handler for the `cx_browse` tool.

use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::validation::clamp_limit;
use cm_core::{ContextStore, EntryKind, ScopePath};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

use super::entry_to_browse_json;

#[derive(Debug, Deserialize)]
struct CxBrowseParams {
    /// Filter to entries at this exact scope path (no ancestor walk).
    #[serde(default)]
    scope_path: Option<String>,

    /// Filter by entry kind.
    #[serde(default)]
    kind: Option<String>,

    /// Filter by tag.
    #[serde(default)]
    tag: Option<String>,

    /// Filter by creator attribution.
    #[serde(default)]
    created_by: Option<String>,

    /// Include superseded entries.
    #[serde(default)]
    include_superseded: bool,

    /// Maximum entries per page.
    #[serde(default)]
    limit: Option<u32>,

    /// Opaque pagination cursor from a previous response.
    #[serde(default)]
    cursor: Option<String>,
}

pub async fn cx_browse(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let params: CxBrowseParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    let scope_path = match &params.scope_path {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    let kind = match &params.kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(cm_err_to_string)?),
        None => None,
    };

    let limit = clamp_limit(params.limit);

    // Delegate to BrowseCapability
    let result = browse::browse(
        store,
        BrowseRequest {
            scope_path,
            kind,
            tag: params.tag,
            created_by: params.created_by,
            include_superseded: params.include_superseded,
            limit,
            cursor: params.cursor,
            ..Default::default()
        },
    )
    .await
    .map_err(cm_err_to_string)?;

    // Map entries through the legacy JSON projection for MCP envelope
    let entries: Vec<Value> = result.entries.iter().map(entry_to_browse_json).collect();

    let response = json!({
        "entries": entries,
        "total": result.total,
        "next_cursor": result.next_cursor,
        "has_more": result.has_more,
    });

    json_response(response)
}
