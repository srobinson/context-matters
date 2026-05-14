//! Handler for the `cx_deposit` tool.
//!
//! Thin MCP adapter: parses JSON params, delegates to the shared
//! [`cm_capabilities::deposit::deposit`] capability, and renders the
//! result through [`format_deposit_ack`]. All validation and write
//! logic lives in the capability so the CLI (`cm deposit`) and MCP
//! (`cx_deposit`) channels surface byte-identical behaviour.

use cm_capabilities::deposit::{self, DepositRequest, Exchange};
use cm_capabilities::projection::format_deposit_ack;
use cm_core::{ContextStore, MutationSource, ScopePath, WriteContext};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
};
use crate::shared::parse_structured_scope_selector;

use super::default_created_by;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CxDepositParams {
    /// Conversation exchanges to store.
    exchanges: Vec<Exchange>,

    /// Optional summary linked to all exchange entries.
    #[serde(default)]
    summary: Option<String>,

    /// Target scope selector. Default: "global".
    #[serde(default)]
    scope: Option<Value>,

    /// Attribution. Default: "agent:claude-code".
    #[serde(default = "default_created_by")]
    created_by: String,
}

pub async fn cx_deposit(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);
    reject_removed_scope_inputs(args)?;
    let params: CxDepositParams = parse_params(args)?;
    let scope = parse_structured_scope_selector(params.scope)?
        .unwrap_or_else(|| cm_capabilities::scope::ScopeSelector::Path(ScopePath::global()));

    let request = DepositRequest {
        exchanges: params.exchanges,
        summary: params.summary,
        scope: Some(scope),
        created_by: params.created_by,
    };

    let result = deposit::deposit(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    let id_strings: Vec<String> = result.entry_ids.iter().map(Uuid::to_string).collect();
    let summary_str = result.summary_id.map(|id| id.to_string());
    let text = format_deposit_ack(&id_strings, summary_str.as_deref(), &result.scope_path);
    let structured = serde_json::json!({
        "deposited": id_strings.len(),
        "entry_ids": id_strings,
        "summary_id": summary_str,
        "scope_path": result.scope_path
    });
    dual_response(text, &structured)
}
