//! Handler for the `cx_deposit` tool.
//!
//! Thin MCP adapter: parses JSON params, delegates to the shared
//! [`cm_capabilities::deposit::deposit`] capability, and renders the
//! result through [`format_deposit_ack`]. All validation and write
//! logic lives in the capability so the CLI (`cm deposit`) and MCP
//! (`cx_deposit`) channels surface byte-identical behaviour.

use cm_capabilities::deposit::{self, DepositRequest, Exchange};
use cm_capabilities::projection::format_deposit_ack;
use cm_capabilities::scope::ScopeSelector;
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::mcp::{ToolResult, cm_err_to_string, parse_params, yaml_response};

use super::{default_created_by, default_scope};

#[derive(Debug, Deserialize)]
struct CxDepositParams {
    /// Conversation exchanges to store.
    exchanges: Vec<Exchange>,

    /// Optional summary linked to all exchange entries.
    #[serde(default)]
    summary: Option<String>,

    /// Target scope path. Default: "global".
    #[serde(default = "default_scope")]
    scope_path: String,

    /// Attribution. Default: "agent:claude-code".
    #[serde(default = "default_created_by")]
    created_by: String,
}

pub async fn cx_deposit(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);
    let params: CxDepositParams = parse_params(args)?;

    let request = DepositRequest {
        exchanges: params.exchanges,
        summary: params.summary,
        scope: Some(ScopeSelector::parse(&params.scope_path).map_err(cm_err_to_string)?),
        created_by: params.created_by,
    };

    let result = deposit::deposit(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    let id_strings: Vec<String> = result.entry_ids.iter().map(Uuid::to_string).collect();
    let summary_str = result.summary_id.map(|id| id.to_string());
    yaml_response(format_deposit_ack(
        &id_strings,
        summary_str.as_deref(),
        &result.scope_path,
    ))
}
