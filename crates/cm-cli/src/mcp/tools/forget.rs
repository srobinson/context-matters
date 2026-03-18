//! Handler for the `cx_forget` tool.

use cm_core::ContextStore;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response};

#[derive(Debug, Deserialize)]
struct CxForgetParams {
    /// Entry IDs to forget. Maximum 100 per request.
    ids: Vec<String>,
}

pub async fn cx_forget(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let params: CxForgetParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

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
    let mut details = Vec::with_capacity(uuids.len());

    for &id in &uuids {
        // Check current state
        match store.get_entry(id).await {
            Ok(entry) => {
                if entry.superseded_by.is_some() {
                    already_inactive += 1;
                    details.push(json!({"id": id.to_string(), "status": "already_inactive"}));
                } else {
                    match store.forget_entry(id).await {
                        Ok(()) => {
                            forgotten += 1;
                            details.push(json!({"id": id.to_string(), "status": "forgotten"}));
                        }
                        Err(e) => {
                            details.push(json!({"id": id.to_string(), "status": "error", "error": cm_err_to_string(e)}));
                        }
                    }
                }
            }
            Err(cm_core::CmError::EntryNotFound(_)) => {
                not_found += 1;
                details.push(json!({"id": id.to_string(), "status": "not_found"}));
            }
            Err(e) => {
                details.push(
                    json!({"id": id.to_string(), "status": "error", "error": cm_err_to_string(e)}),
                );
            }
        }
    }

    let mut parts = Vec::new();
    if forgotten > 0 {
        parts.push(format!("Forgot {forgotten} entries."));
    }
    if already_inactive > 0 {
        parts.push(format!("{already_inactive} already inactive."));
    }
    if not_found > 0 {
        parts.push(format!("{not_found} not found."));
    }
    let message = if parts.is_empty() {
        "No entries processed.".to_owned()
    } else {
        parts.join(" ")
    };

    let response = json!({
        "forgotten": forgotten,
        "already_inactive": already_inactive,
        "not_found": not_found,
        "details": details,
        "message": message,
    });

    json_response(response)
}
