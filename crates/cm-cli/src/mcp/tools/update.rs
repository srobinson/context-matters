//! Handler for the `cx_update` tool.

use cm_core::{ContextStore, EntryKind, EntryMeta, MutationSource, UpdateEntry, WriteContext};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{check_input_size, cm_err_to_string, json_response};

use super::parse_confidence;

#[derive(Debug, Deserialize)]
struct CxUpdateParams {
    /// ID of the entry to update.
    id: String,

    /// New title.
    #[serde(default)]
    title: Option<String>,

    /// New body content.
    #[serde(default)]
    body: Option<String>,

    /// New kind classification.
    #[serde(default)]
    kind: Option<String>,

    /// Replace metadata entirely.
    #[serde(default)]
    meta: Option<CxMetaInput>,
}

#[derive(Debug, Deserialize)]
struct CxMetaInput {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    priority: Option<i32>,
}

pub async fn cx_update(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let params: CxUpdateParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    let id = uuid::Uuid::parse_str(&params.id)
        .map_err(|_| format!("Invalid UUID format: '{}'", params.id))?;

    // Validate at least one field is provided
    if params.title.is_none()
        && params.body.is_none()
        && params.kind.is_none()
        && params.meta.is_none()
    {
        return Err("Validation error: at least one field must be provided".to_owned());
    }

    // Validate input sizes
    if let Some(ref t) = params.title {
        check_input_size(t, "title")?;
    }
    if let Some(ref b) = params.body {
        check_input_size(b, "body")?;
    }

    // Parse kind if provided
    let kind = match &params.kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(cm_err_to_string)?),
        None => None,
    };

    // Parse meta if provided
    let meta = match params.meta {
        Some(m) => {
            let confidence = match &m.confidence {
                Some(c) => Some(parse_confidence(c)?),
                None => None,
            };
            let expires_at = match &m.expires_at {
                Some(s) => Some(
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|e| {
                            format!("Invalid expires_at: {e}. Expected ISO 8601 format.")
                        })?,
                ),
                None => None,
            };
            Some(EntryMeta {
                tags: m.tags,
                confidence,
                source: m.source,
                expires_at,
                priority: m.priority,
                extra: std::collections::HashMap::new(),
            })
        }
        None => None,
    };

    let update = UpdateEntry {
        title: params.title,
        body: params.body,
        kind,
        meta,
    };

    let ctx = WriteContext::new(MutationSource::Mcp);

    let entry = store
        .update_entry(id, update, &ctx)
        .await
        .map_err(cm_err_to_string)?;

    let response = json!({
        "entry": {
            "id": entry.id.to_string(),
            "scope_path": entry.scope_path.as_str(),
            "kind": entry.kind.as_str(),
            "title": &entry.title,
            "content_hash": &entry.content_hash,
            "updated_at": entry.updated_at.to_rfc3339(),
        },
        "message": "Entry updated.",
    });

    json_response(response)
}
