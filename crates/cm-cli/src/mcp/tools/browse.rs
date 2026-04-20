//! Handler for the `cx_browse` tool.

use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{format_browse_view, project_web_browse};
use cm_capabilities::scope::BrowseScopeMode;
use cm_capabilities::validation::clamp_limit;
use cm_core::{ContextStore, EntryKind, ScopePath};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, dual_response, parse_params};

#[derive(Debug, Deserialize)]
struct CxBrowseParams {
    /// Preferred scope input. Accepts "auto" for local scope inference
    /// or an explicit scope path for exact filtering.
    #[serde(default)]
    scope: Option<String>,

    /// Filter to entries at this exact scope path (no ancestor walk).
    #[serde(default)]
    scope_path: Option<String>,

    /// Browse scope resolution mode. Only "resolved" is supported.
    #[serde(default)]
    scope_mode: Option<String>,

    /// Filesystem cwd used for scope="auto" inference.
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
    let params: CxBrowseParams = parse_params(args)?;

    let scope = params.scope.or_else(|| {
        if params.scope_path.is_none() {
            Some("auto".to_owned())
        } else {
            None
        }
    });
    let scope_is_auto = matches!(scope.as_deref().map(str::trim), Some("auto"));

    let scope_path = match &params.scope_path {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    let scope_mode = match &params.scope_mode {
        Some(mode) => mode.parse::<BrowseScopeMode>().map_err(cm_err_to_string)?,
        None => BrowseScopeMode::default(),
    };

    let cwd = match params.cwd {
        Some(raw) if raw.trim().is_empty() => {
            return Err("Invalid parameters: cwd cannot be empty".to_owned());
        }
        Some(raw) => Some(raw.into()),
        None if scope_is_auto => Some(
            std::env::current_dir()
                .map_err(|e| format!("Failed to determine current working directory: {e}"))?,
        ),
        None => None,
    };

    let kind = match &params.kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(cm_err_to_string)?),
        None => None,
    };

    let limit = clamp_limit(params.limit);

    let request = BrowseRequest {
        scope,
        scope_path,
        scope_mode,
        cwd,
        include_resolution: params.include_resolution.unwrap_or(scope_is_auto),
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
    let mut view = project_web_browse(&result);
    if !request.include_resolution {
        view.resolution = None;
    }
    dual_response(text, &view)
}
