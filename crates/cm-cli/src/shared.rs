//! Reusable helpers shared between cm-cli (MCP server) and cm-web (HTTP API).
//!
//! Error conversion, input validation, entry formatting, scope management.

use cm_core::{
    CmError, ContextStore, Entry, EntryFilter, EntryKind, Pagination, ScopePath, WriteContext,
};
use serde_json::Value;

// ── Re-exports from cm-capabilities ──────────────────────────────

pub use cm_capabilities::constants::{
    DEFAULT_LIMIT, MAX_BATCH_IDS, MAX_INPUT_BYTES, MAX_LIMIT, SNIPPET_LENGTH,
};
pub use cm_capabilities::projection::{
    BrowseEntryView, FullEntryView, RecallEntryView, entry_has_any_tag, estimate_tokens,
    project_browse_entry, project_full_entry, project_recall_entry, snippet,
};
pub use cm_capabilities::validation::{check_input_size, clamp_limit, parse_confidence};

// ── Error Conversion ──────────────────────────────────────────────

/// Convert a `CmError` to an actionable error string with recovery guidance.
pub fn cm_err_to_string(e: CmError) -> String {
    match e {
        CmError::EntryNotFound(id) => {
            format!("Entry '{id}' not found. Verify the ID using cx_browse or cx_recall.")
        }
        CmError::ScopeNotFound(path) => {
            format!(
                "Scope '{path}' does not exist. Use cx_stats to list available scopes, \
                 or create it by storing an entry with a new scope_path."
            )
        }
        CmError::DuplicateContent(existing_id) => {
            format!(
                "Duplicate content: an active entry with this content already exists \
                 (id: {existing_id}). Use cx_update to modify the existing entry, \
                 or cx_forget it first."
            )
        }
        CmError::InvalidScopePath(e) => {
            format!(
                "Invalid scope_path: {e}. Format: 'global', 'global/project:<id>', \
                 'global/project:<id>/repo:<id>', or \
                 'global/project:<id>/repo:<id>/session:<id>'. \
                 Identifiers must be lowercase alphanumeric with hyphens."
            )
        }
        CmError::InvalidEntryKind(s) => {
            format!(
                "Invalid kind '{s}'. Valid values: fact, decision, preference, lesson, \
                 reference, feedback, pattern, observation."
            )
        }
        CmError::InvalidRelationKind(s) => {
            format!(
                "Invalid relation kind '{s}'. Valid values: supersedes, relates_to, \
                 contradicts, elaborates, depends_on."
            )
        }
        CmError::Validation(msg) => msg,
        CmError::ConstraintViolation(msg) => format!("Constraint violation: {msg}"),
        CmError::Json(e) => format!("[json] {e}"),
        CmError::Database(msg) => format!("[database] {msg}"),
        CmError::Internal(msg) => format!("[internal] {msg}"),
    }
}

// ── Input Validation (re-exported from cm-capabilities above) ────

/// Serialize a JSON value to a pretty-printed string for the response.
pub fn json_response(value: Value) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|e| format!("[json] {e}"))
}

// ── Serde Defaults ────────────────────────────────────────────────

/// Serde default for scope_path fields.
pub fn default_scope() -> String {
    "global".to_owned()
}

/// Serde default for created_by fields.
pub fn default_created_by() -> String {
    "agent:claude-code".to_owned()
}

// ── Entry Formatting (legacy JSON wrappers, delegates to cm-capabilities) ──

/// Convert an entry to the two-phase recall response format (snippet, not full body).
pub fn entry_to_recall_json(entry: &Entry) -> Value {
    serde_json::to_value(project_recall_entry(entry)).expect("RecallEntryView serializes")
}

/// Convert an entry to the browse response format (two-phase: snippet, not full body).
pub fn entry_to_browse_json(entry: &Entry) -> Value {
    serde_json::to_value(project_browse_entry(entry)).expect("BrowseEntryView serializes")
}

/// Convert an entry to the full response format (includes body).
pub fn entry_to_full_json(entry: &Entry) -> Value {
    serde_json::to_value(project_full_entry(entry)).expect("FullEntryView serializes")
}

/// Browse through scopes and pages until enough no-query recall matches are found.
///
/// This preserves recall semantics for scoped ancestor walks while avoiding
/// the false negatives caused by fetching one widened page and post-filtering it.
pub async fn recall_candidates_without_query(
    store: &impl ContextStore,
    scope_path: Option<&ScopePath>,
    kind_filters: &[EntryKind],
    tags: &[String],
    limit: u32,
) -> Result<Vec<Entry>, CmError> {
    let scoped_paths: Vec<Option<ScopePath>> = match scope_path {
        Some(scope_path) => scope_path
            .ancestors()
            .map(|path| ScopePath::parse(path).expect("validated ancestor path"))
            .map(Some)
            .collect(),
        None => vec![None],
    };

    let direct_kind = if kind_filters.len() == 1 {
        Some(kind_filters[0])
    } else {
        None
    };
    let direct_tag = (tags.len() == 1).then(|| tags[0].clone());
    let mut matched = Vec::new();

    for scoped_path in scoped_paths {
        let mut cursor = None;

        loop {
            let page = store
                .browse(EntryFilter {
                    scope_path: scoped_path.clone(),
                    kind: direct_kind,
                    tag: direct_tag.clone(),
                    pagination: Pagination {
                        limit: MAX_LIMIT,
                        cursor,
                    },
                    ..Default::default()
                })
                .await?;

            for entry in page.items {
                let kind_ok = kind_filters.is_empty() || kind_filters.contains(&entry.kind);
                let tag_ok = tags.is_empty() || entry_has_any_tag(&entry, tags);

                if kind_ok && tag_ok {
                    matched.push(entry);
                    if matched.len() >= limit as usize {
                        return Ok(matched);
                    }
                }
            }

            let Some(next_cursor) = page.next_cursor else {
                break;
            };
            cursor = Some(next_cursor);
        }
    }

    Ok(matched)
}

// ── Scope Management ──────────────────────────────────────────────

/// Ensure the full scope chain exists, creating missing scopes top-down.
///
/// When creating an entry with a scope path that does not exist, this
/// function creates the full scope chain automatically. This prevents
/// callers from needing to manage scope creation separately.
pub async fn ensure_scope_chain(
    store: &impl ContextStore,
    path: &ScopePath,
    ctx: &WriteContext,
) -> Result<(), String> {
    use cm_core::NewScope;

    let ancestors: Vec<&str> = path.ancestors().collect();

    // Walk from root (last) to leaf (first)
    for ancestor_str in ancestors.into_iter().rev() {
        let ancestor = ScopePath::parse(ancestor_str).map_err(|e| cm_err_to_string(e.into()))?;
        match store.get_scope(&ancestor).await {
            Ok(_) => continue,
            Err(CmError::ScopeNotFound(_)) => {
                // Derive label from the last segment
                let label = ancestor_str
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.split(':').nth(1))
                    .unwrap_or(ancestor_str)
                    .to_owned();

                let new_scope = NewScope {
                    path: ancestor,
                    label,
                    meta: None,
                };
                store
                    .create_scope(new_scope, ctx)
                    .await
                    .map_err(cm_err_to_string)?;
            }
            Err(e) => return Err(cm_err_to_string(e)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cm_err_to_string_includes_recovery_guidance() {
        let err = CmError::EntryNotFound(uuid::Uuid::nil());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("cx_browse"));
        assert!(msg.contains("cx_recall"));

        let err = CmError::InvalidEntryKind("bogus".to_owned());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("fact"));
        assert!(msg.contains("decision"));
        assert!(msg.contains("observation"));
    }

    #[test]
    fn cm_err_to_string_scope_not_found_has_guidance() {
        let err = CmError::ScopeNotFound("global/project:foo".to_owned());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("cx_stats"));
    }

    #[test]
    fn cm_err_to_string_duplicate_has_guidance() {
        let err = CmError::DuplicateContent(uuid::Uuid::nil());
        let msg = cm_err_to_string(err);
        assert!(msg.contains("cx_update"));
        assert!(msg.contains("cx_forget"));
    }
}
