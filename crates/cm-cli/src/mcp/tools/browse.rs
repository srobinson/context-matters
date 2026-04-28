//! Handler for the `cx_browse` tool.

use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{format_browse_view, project_web_browse};
use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::validation::parse_kind;
use cm_core::ContextStore;
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CxBrowseParams {
    /// Preferred scope input. Accepts "cwd_inferred" for local scope
    /// inference or an explicit scope path for exact filtering.
    #[serde(default)]
    scope: Option<String>,

    /// Filesystem cwd used for scope="cwd_inferred" inference.
    #[serde(default)]
    cwd: Option<String>,

    /// Include resolution metadata in projected responses.
    #[serde(default)]
    include_resolution: Option<bool>,

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
    reject_removed_scope_inputs(args)?;
    let params: CxBrowseParams = parse_params(args)?;

    let cwd = match params.cwd {
        Some(raw) if raw.trim().is_empty() => {
            return Err("Invalid parameters: cwd cannot be empty".to_owned());
        }
        Some(raw) => Some(raw.into()),
        None => None,
    };
    let scope = ScopeSelector::from_optional_scope(params.scope.as_deref(), cwd)
        .map_err(cm_err_to_string)?;

    let kind = match &params.kind {
        Some(k) => Some(parse_kind(k)?),
        None => None,
    };

    let request = BrowseRequest {
        scope,
        include_resolution: params.include_resolution,
        kind,
        tag: params.tag,
        created_by: params.created_by,
        include_superseded: params.include_superseded,
        limit: params.limit,
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
