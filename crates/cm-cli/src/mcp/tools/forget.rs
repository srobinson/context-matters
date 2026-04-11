//! Handler for the `cx_forget` tool.

use cm_capabilities::projection::{ForgetError, format_forget_ack};
use cm_core::{ContextStore, MutationSource, WriteContext};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::{ToolResult, cm_err_to_string, parse_params, yaml_response};

#[derive(Debug, Deserialize)]
struct CxForgetParams {
    /// Entry IDs to forget. Maximum 100 per request.
    ids: Vec<String>,
}

pub async fn cx_forget(store: &impl ContextStore, args: &Value) -> Result<ToolResult, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);

    let params: CxForgetParams = parse_params(args)?;

    if params.ids.is_empty() {
        return Err("Validation error: ids cannot be empty".to_owned());
    }
    if params.ids.len() > crate::mcp::MAX_BATCH_IDS {
        return Err(format!(
            "Validation error: maximum {} IDs per request",
            crate::mcp::MAX_BATCH_IDS
        ));
    }

    let uuids: Vec<uuid::Uuid> = params
        .ids
        .iter()
        .map(|s| uuid::Uuid::parse_str(s).map_err(|_| format!("Invalid UUID format: '{s}'")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut forgotten = 0u32;
    let mut already_inactive = 0u32;
    let mut not_found = 0u32;
    let mut errors: Vec<ForgetError> = Vec::new();

    for &id in &uuids {
        // Check current state
        match store.get_entry(id).await {
            Ok(entry) => {
                if entry.superseded_by.is_some() {
                    already_inactive += 1;
                } else {
                    match store.forget_entry(id, &ctx).await {
                        Ok(()) => {
                            forgotten += 1;
                        }
                        Err(e) => {
                            errors.push(ForgetError {
                                id: id.to_string(),
                                error: cm_err_to_string(e),
                            });
                        }
                    }
                }
            }
            Err(cm_core::CmError::EntryNotFound(_)) => {
                not_found += 1;
            }
            Err(e) => {
                errors.push(ForgetError {
                    id: id.to_string(),
                    error: cm_err_to_string(e),
                });
            }
        }
    }

    yaml_response(format_forget_ack(
        forgotten,
        already_inactive,
        not_found,
        &errors,
    ))
}
