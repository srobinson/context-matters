//! Handler for the `cx_get` tool.

use cm_core::ContextStore;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{cm_err_to_string, json_response, parse_params};

use super::entry_to_full_json;

#[derive(Debug, Deserialize)]
struct CxGetParams {
    /// Entry IDs to retrieve. Maximum 100 per request.
    ids: Vec<String>,
}

pub async fn cx_get(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let params: CxGetParams = parse_params(args)?;

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

    let entries = store.get_entries(&uuids).await.map_err(cm_err_to_string)?;
    let found = entries.len();
    let missing = uuids.len() - found;

    let entries_json: Vec<Value> = entries.iter().map(entry_to_full_json).collect();

    let response = json!({
        "entries": entries_json,
        "found": found,
        "missing": missing,
    });

    json_response(response)
}
