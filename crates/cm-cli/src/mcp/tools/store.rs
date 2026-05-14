//! Handler for the `cx_store` tool.

use cm_capabilities::projection::format_store_ack;
use cm_capabilities::store::{StoreRequest, store as store_entry};
use cm_capabilities::validation::MetaInput;
use cm_core::{ContextStore, MutationSource, ScopePath, WriteContext};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{
    ToolResult, cm_err_to_string, dual_response, parse_params, reject_removed_scope_inputs,
    reject_unknown_fields,
};
use crate::shared::parse_structured_scope_selector;

use super::default_created_by;

#[derive(Debug, Deserialize)]
struct CxStoreParams {
    title: String,
    body: String,
    kind: String,
    #[serde(default)]
    scope: Option<Value>,
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
    let scope = parse_structured_scope_selector(params.scope)?
        .unwrap_or_else(|| cm_capabilities::scope::ScopeSelector::Path(ScopePath::global()));
    let request = StoreRequest {
        title: params.title,
        body: params.body,
        kind: params.kind,
        scope: Some(scope),
        created_by: params.created_by,
        meta: params.meta,
        supersedes: params.supersedes,
    };

    let result = store_entry(store, request, &ctx)
        .await
        .map_err(cm_err_to_string)?;
    let text = format_store_ack(
        &result.entry_id,
        &result.scope_path,
        result.kind.as_str(),
        &result.content_hash,
        result.superseded_id.as_deref(),
    );
    let structured = serde_json::json!({
        "id": result.entry_id,
        "scope_path": result.scope_path,
        "kind": result.kind.as_str(),
        "content_hash": result.content_hash,
        "superseded_id": result.superseded_id,
        "scope_created": result.scope_created
    });
    dual_response(text, &structured)
}
