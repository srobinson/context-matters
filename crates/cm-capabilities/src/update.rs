//! Shared update capability for CLI and MCP adapters.

use cm_core::{CmError, ContextStore, UpdateEntry, WriteContext};
use serde::{Deserialize, Serialize};

use crate::validation::{MetaInput, check_input_size, parse_kind, parse_uuid};

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRequest {
    /// ID of the entry to update.
    pub id: String,
    /// New title.
    #[serde(default)]
    pub title: Option<String>,
    /// New body content.
    #[serde(default)]
    pub body: Option<String>,
    /// New kind classification.
    #[serde(default)]
    pub kind: Option<String>,
    /// Replace metadata entirely.
    #[serde(default)]
    pub meta: Option<MetaInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateResult {
    pub updated_id: String,
    pub content_hash: String,
}

pub async fn update(
    store: &impl ContextStore,
    request: UpdateRequest,
    ctx: &WriteContext,
) -> Result<UpdateResult, CmError> {
    let id = parse_uuid(&request.id).map_err(CmError::Validation)?;

    if request.title.is_none()
        && request.body.is_none()
        && request.kind.is_none()
        && request.meta.is_none()
    {
        return Err(CmError::Validation(
            "at least one field must be provided: title, body, kind, or meta".to_owned(),
        ));
    }

    if let Some(ref title) = request.title {
        check_input_size(title, "title").map_err(CmError::Validation)?;
    }
    if let Some(ref body) = request.body {
        check_input_size(body, "body").map_err(CmError::Validation)?;
    }

    let kind = match request.kind {
        Some(ref kind) => Some(parse_kind(kind).map_err(CmError::Validation)?),
        None => None,
    };

    let meta = match request.meta {
        Some(meta) => Some(meta.into_entry_meta().map_err(CmError::Validation)?),
        None => None,
    };

    let entry = store
        .update_entry(
            id,
            UpdateEntry {
                title: request.title,
                body: request.body,
                kind,
                meta,
            },
            ctx,
        )
        .await?;

    Ok(UpdateResult {
        updated_id: entry.id.to_string(),
        content_hash: entry.content_hash,
    })
}
