//! Store capability: create or supersede an entry.
//!
//! The store flow owns input validation, request defaults, metadata parsing,
//! scope chain auto-creation, and supersedes parsing so adapters can stay thin.

use cm_core::{CmError, ContextStore, EntryKind, NewEntry, ScopePath, WriteContext};
use serde::Deserialize;

use crate::scope::ensure_scope_chain_with_status;
use crate::validation::{MetaInput, check_input_size, parse_kind};

const DEFAULT_SCOPE_PATH: &str = "global";
const DEFAULT_CREATED_BY: &str = "agent:claude-code";

fn default_scope_path() -> String {
    DEFAULT_SCOPE_PATH.to_owned()
}

fn default_created_by() -> String {
    DEFAULT_CREATED_BY.to_owned()
}

/// Input for a store operation.
#[derive(Debug, Clone, Deserialize)]
pub struct StoreRequest {
    /// Short summary displayed in search results.
    pub title: String,
    /// Full content body in markdown.
    pub body: String,
    /// Entry classification.
    pub kind: String,
    /// Target scope path. Auto-created if missing.
    #[serde(default = "default_scope_path")]
    pub scope_path: String,
    /// Attribution string.
    #[serde(default = "default_created_by")]
    pub created_by: String,
    /// Metadata fields accepted as top-level store parameters.
    #[serde(flatten)]
    pub meta: MetaInput,
    /// ID of an existing entry that this new entry supersedes.
    #[serde(default)]
    pub supersedes: Option<String>,
}

/// Result of a store operation.
#[derive(Debug, Clone)]
pub struct StoreResult {
    pub entry_id: String,
    pub content_hash: String,
    pub scope_path: String,
    pub kind: EntryKind,
    pub superseded_id: Option<String>,
    pub scope_created: bool,
}

/// Create a new entry, or supersede an existing entry with a replacement.
pub async fn store(
    store: &impl ContextStore,
    request: StoreRequest,
    ctx: &WriteContext,
) -> Result<StoreResult, CmError> {
    check_input_size(&request.title, "title").map_err(CmError::Validation)?;
    check_input_size(&request.body, "body").map_err(CmError::Validation)?;

    let scope_path = ScopePath::parse(&request.scope_path)?;
    let kind = parse_kind(&request.kind).map_err(CmError::Validation)?;

    let meta = if request.meta.is_empty() {
        None
    } else {
        Some(
            request
                .meta
                .into_entry_meta()
                .map_err(CmError::Validation)?,
        )
    };

    let scope_created = ensure_scope_chain_with_status(store, &scope_path, ctx).await?;

    let supersedes = match request.supersedes {
        Some(ref id_str) => Some(uuid::Uuid::parse_str(id_str).map_err(|_| {
            CmError::Validation(format!(
                "Invalid supersedes ID: '{id_str}'. Expected a UUID."
            ))
        })?),
        None => None,
    };

    let new_entry = NewEntry {
        scope_path,
        kind,
        title: request.title,
        body: request.body,
        created_by: request.created_by,
        meta,
    };

    let entry = match supersedes {
        Some(old_id) => store.supersede_entry(old_id, new_entry, ctx).await?,
        None => store.create_entry(new_entry, ctx).await?,
    };

    Ok(StoreResult {
        entry_id: entry.id.to_string(),
        content_hash: entry.content_hash,
        scope_path: entry.scope_path.as_str().to_owned(),
        kind: entry.kind,
        superseded_id: supersedes.map(|id| id.to_string()),
        scope_created,
    })
}
