//! Handler for the `cx_store` tool.

use cm_core::{ContextStore, EntryKind, EntryMeta, NewEntry, ScopePath};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{check_input_size, cm_err_to_string, ensure_scope_chain, json_response};

use super::{default_created_by, default_scope, parse_confidence};

/// Parameters for the `cx_store` tool.
#[derive(Debug, Deserialize)]
struct CxStoreParams {
    /// Short summary displayed in search results.
    title: String,

    /// Full content body in markdown.
    body: String,

    /// Entry classification.
    kind: String,

    /// Target scope path. Auto-created if missing.
    #[serde(default = "default_scope")]
    scope_path: String,

    /// Attribution string.
    #[serde(default = "default_created_by")]
    created_by: String,

    /// Freeform tags.
    #[serde(default)]
    tags: Vec<String>,

    /// Confidence level: "high", "medium", or "low".
    #[serde(default)]
    confidence: Option<String>,

    /// Source URL or path.
    #[serde(default)]
    source: Option<String>,

    /// ISO 8601 expiry timestamp.
    #[serde(default)]
    expires_at: Option<String>,

    /// Numeric priority for manual ordering.
    #[serde(default)]
    priority: Option<i32>,

    /// ID of an existing entry that this new entry supersedes.
    #[serde(default)]
    supersedes: Option<String>,
}

pub async fn cx_store(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let params: CxStoreParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    // Validate input sizes
    check_input_size(&params.title, "title")?;
    check_input_size(&params.body, "body")?;

    // Parse scope path and entry kind
    let scope_path =
        ScopePath::parse(&params.scope_path).map_err(|e| cm_err_to_string(e.into()))?;
    let kind: EntryKind = params.kind.parse().map_err(cm_err_to_string)?;

    // Parse confidence if provided
    let confidence = match &params.confidence {
        Some(c) => Some(parse_confidence(c)?),
        None => None,
    };

    // Parse expires_at if provided
    let expires_at = match &params.expires_at {
        Some(s) => Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| format!("Invalid expires_at: {e}. Expected ISO 8601 format."))?,
        ),
        None => None,
    };

    // Auto-create scope chain if needed
    ensure_scope_chain(store, &scope_path).await?;

    // Build metadata
    let meta = if !params.tags.is_empty()
        || confidence.is_some()
        || params.source.is_some()
        || expires_at.is_some()
        || params.priority.is_some()
    {
        Some(EntryMeta {
            tags: params.tags,
            confidence,
            source: params.source,
            expires_at,
            priority: params.priority,
            extra: std::collections::HashMap::new(),
        })
    } else {
        None
    };

    let new_entry = NewEntry {
        scope_path,
        kind,
        title: params.title,
        body: params.body,
        created_by: params.created_by,
        meta,
    };

    // Create or supersede
    let (entry, superseded_id) = match params.supersedes {
        Some(ref id_str) => {
            let old_id = uuid::Uuid::parse_str(id_str)
                .map_err(|_| format!("Invalid supersedes ID: '{id_str}'. Expected a UUID."))?;
            let entry = store
                .supersede_entry(old_id, new_entry)
                .await
                .map_err(cm_err_to_string)?;
            (entry, Some(old_id))
        }
        None => {
            let entry = store
                .create_entry(new_entry)
                .await
                .map_err(cm_err_to_string)?;
            (entry, None)
        }
    };

    let message = match superseded_id {
        Some(old_id) => format!("Entry stored. Superseded entry {old_id}."),
        None => "Entry stored.".to_owned(),
    };

    let response = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "content_hash": &entry.content_hash,
        "created_at": entry.created_at.to_rfc3339(),
        "superseded": superseded_id.map(|id| id.to_string()),
        "message": message,
    });

    json_response(response)
}
