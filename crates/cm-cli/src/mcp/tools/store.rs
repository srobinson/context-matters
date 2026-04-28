//! Handler for the `cx_store` tool.

use cm_capabilities::projection::format_store_ack;
use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::store::{StoreRequest, store as store_entry};
use cm_capabilities::validation::MetaInput;
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, parse_params, reject_removed_scope_inputs, reject_unknown_fields,
    yaml_response,
};

use super::{default_created_by, default_scope};

#[derive(Debug, Deserialize)]
struct CxStoreParams {
    title: String,
    body: String,
    kind: String,
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default = "default_created_by")]
    created_by: String,
    #[serde(flatten)]
    meta: MetaInput,
    #[serde(default)]
    supersedes: Option<String>,
}

pub async fn cx_store(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);
    reject_removed_scope_inputs(args)?;
    reject_unknown_fields(
        args,
        &[
            "title",
            "body",
            "kind",
            "scope",
            "created_by",
            "tags",
            "confidence",
            "source",
            "expires_at",
            "priority",
            "supersedes",
        ],
    )?;
    let params: CxStoreParams = parse_params(args)?;
    let request = StoreRequest {
        title: params.title,
        body: params.body,
        kind: params.kind,
        scope: Some(ScopeSelector::parse(&params.scope).map_err(cm_err_to_string)?),
        created_by: params.created_by,
        meta: params.meta,
        supersedes: params.supersedes,
    };

    let result = store_entry(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;
    yaml_response(format_store_ack(
        &result.entry_id,
        &result.scope_path,
        result.kind.as_str(),
        &result.content_hash,
        result.superseded_id.as_deref(),
    ))
}
