//! Get capability: fetch full entries by ID in batch.
//!
//! This keeps UUID parsing, batch-size validation, and canonical requested-id
//! tracking in the capability layer so CLI and MCP adapters can stay thin.

use cm_core::{CmError, ContextStore, Entry};
use serde::Deserialize;

use crate::validation::parse_uuid_batch;

/// Input for a get operation.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GetRequest {
    /// Entry IDs to retrieve. Empty and over-large batches are rejected
    /// before the store is touched.
    #[serde(default)]
    pub ids: Vec<String>,
}

/// Result of a get operation.
#[derive(Debug, Clone)]
pub struct GetResult {
    /// Entries found by the store, preserving request order for found IDs.
    pub entries: Vec<Entry>,
    /// Canonical requested IDs in caller order. Projection uses this list to
    /// compute and render missing IDs.
    pub requested_ids: Vec<String>,
}

/// Fetch full entries for the requested IDs.
///
/// Missing IDs are not errors. The underlying store omits them, and the
/// projection layer reports them by diffing `requested_ids` against returned
/// entries.
pub async fn get(store: &impl ContextStore, request: GetRequest) -> Result<GetResult, CmError> {
    let parsed = parse_uuid_batch(&request.ids).map_err(CmError::Validation)?;
    let entries = store.get_entries(&parsed.uuids).await?;

    Ok(GetResult {
        entries,
        requested_ids: parsed.canonical_ids,
    })
}
