//! Handler for the `cx_export` tool.

use cm_core::{ContextStore, ScopePath};
use cm_store::CmStore;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

#[derive(Debug, Deserialize)]
struct CxExportParams {
    /// Filter to a specific scope path.
    #[serde(default)]
    scope_path: Option<String>,

    /// Export format. Only "json" supported.
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "json".to_owned()
}

pub fn cx_export(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxExportParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    if params.format != "json" {
        return Err(format!(
            "Unsupported export format '{}'. Currently only 'json' is supported.",
            params.format
        ));
    }

    let scope_path = match &params.scope_path {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    let entries = store
        .export(scope_path.as_ref())
        .map_err(cm_err_to_string)?;

    let all_scopes = store.list_scopes(None).map_err(cm_err_to_string)?;

    // Filter scopes by prefix if scope_path is specified
    let scopes: Vec<_> = match &scope_path {
        Some(sp) => all_scopes
            .into_iter()
            .filter(|s| s.path.as_str().starts_with(sp.as_str()))
            .collect(),
        None => all_scopes,
    };

    let count = entries.len();

    let response = json!({
        "entries": entries,
        "scopes": scopes,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "count": count,
    });

    json_response(response)
}
