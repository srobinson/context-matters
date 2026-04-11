//! Handler for the `cx_browse` tool.

use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{format_browse_view, project_web_browse};
use cm_capabilities::validation::clamp_limit;
use cm_core::{ContextStore, EntryKind, ScopePath};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, dual_response, parse_params};

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

pub async fn cx_browse(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let params: CxBrowseParams = parse_params(args)?;

    let scope_path = match &params.scope_path {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    let kind = match &params.kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(cm_err_to_string)?),
        None => None,
    };

    let limit = clamp_limit(params.limit);

    let request = BrowseRequest {
        scope_path,
        kind,
        tag: params.tag,
        created_by: params.created_by,
        include_superseded: params.include_superseded,
        limit,
        cursor: params.cursor,
        ..Default::default()
    };

    let result = browse::browse(store, request.clone())
        .await
        .map_err(cm_err_to_string)?;

    let text = format_browse_view(&result, &request);
    let view = project_web_browse(&result);
    dual_response(text, &view)
}
