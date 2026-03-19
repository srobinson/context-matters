//! Reusable helpers shared between cm-cli (MCP server) and cm-web (HTTP API).
//!
//! Error conversion, input validation, entry formatting, scope management.

use cm_core::{CmError, Confidence, ContextStore, Entry, ScopePath, WriteContext};
use serde_json::{Value, json};

// ── Constants ─────────────────────────────────────────────────────

/// Maximum input size for text-accepting tool fields (1 MB).
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// Maximum number of IDs in a batch request.
pub const MAX_BATCH_IDS: usize = 100;

/// Default result limit for retrieval tools.
pub const DEFAULT_LIMIT: u32 = 20;

/// Maximum result limit.
pub const MAX_LIMIT: u32 = 200;

/// Snippet length for two-phase retrieval responses.
pub const SNIPPET_LENGTH: usize = 200;

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

// ── Input Validation ──────────────────────────────────────────────

/// Reject input exceeding the per-field byte limit.
pub fn check_input_size(value: &str, field: &str) -> Result<(), String> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(format!("{field} exceeds {MAX_INPUT_BYTES} byte limit"));
    }
    Ok(())
}

/// Clamp a limit value to the allowed range `[1, MAX_LIMIT]`.
pub fn clamp_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

// ── Text Helpers ──────────────────────────────────────────────────

/// Truncate body to a snippet, safe for multi-byte UTF-8.
///
/// Uses `floor_char_boundary` (stable since Rust 1.82) to avoid
/// panicking on multi-byte character boundaries. Tries to break
/// at a word boundary for readability.
pub fn snippet(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_owned();
    }
    let end = body.floor_char_boundary(max_bytes);
    match body[..end].rfind(' ') {
        Some(pos) => format!("{}...", &body[..pos]),
        None => format!("{}...", &body[..end]),
    }
}

/// Rough token estimate: ~4 characters per token for English text.
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(4)
}

/// Serialize a JSON value to a pretty-printed string for the response.
pub fn json_response(value: Value) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|e| format!("[json] {e}"))
}

// ── Confidence ────────────────────────────────────────────────────

/// Parse a confidence string to the Confidence enum.
pub fn parse_confidence(s: &str) -> Result<Confidence, String> {
    match s {
        "high" => Ok(Confidence::High),
        "medium" => Ok(Confidence::Medium),
        "low" => Ok(Confidence::Low),
        other => Err(format!(
            "Invalid confidence '{other}'. Valid values: high, medium, low."
        )),
    }
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

// ── Entry Formatting ──────────────────────────────────────────────

/// Convert an entry to the two-phase recall response format (snippet, not full body).
pub fn entry_to_recall_json(entry: &Entry) -> Value {
    let mut result = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "snippet": snippet(&entry.body, SNIPPET_LENGTH),
        "created_by": &entry.created_by,
        "updated_at": entry.updated_at.to_rfc3339(),
    });

    if let Some(ref meta) = entry.meta {
        if !meta.tags.is_empty() {
            result["tags"] = json!(meta.tags);
        }
        if let Some(ref confidence) = meta.confidence {
            result["confidence"] = json!(confidence);
        }
    }

    result
}

/// Convert an entry to the browse response format (two-phase: snippet, not full body).
pub fn entry_to_browse_json(entry: &Entry) -> Value {
    let mut result = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "snippet": snippet(&entry.body, SNIPPET_LENGTH),
        "created_by": &entry.created_by,
        "created_at": entry.created_at.to_rfc3339(),
        "updated_at": entry.updated_at.to_rfc3339(),
        "superseded_by": entry.superseded_by.map(|id| id.to_string()),
    });

    if let Some(ref meta) = entry.meta
        && !meta.tags.is_empty()
    {
        result["tags"] = json!(meta.tags);
    }

    result
}

/// Convert an entry to the full response format (includes body).
pub fn entry_to_full_json(entry: &Entry) -> Value {
    json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "body": &entry.body,
        "content_hash": &entry.content_hash,
        "meta": &entry.meta,
        "created_by": &entry.created_by,
        "created_at": entry.created_at.to_rfc3339(),
        "updated_at": entry.updated_at.to_rfc3339(),
        "superseded_by": entry.superseded_by.map(|id| id.to_string()),
    })
}

/// Check whether an entry has any of the specified tags.
pub fn entry_has_any_tag(entry: &Entry, tags: &[String]) -> bool {
    match &entry.meta {
        Some(meta) => meta.tags.iter().any(|t| tags.contains(t)),
        None => false,
    }
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
    fn clamp_limit_defaults_to_20() {
        assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
    }

    #[test]
    fn clamp_limit_caps_at_max() {
        assert_eq!(clamp_limit(Some(500)), MAX_LIMIT);
    }

    #[test]
    fn clamp_limit_floors_at_1() {
        assert_eq!(clamp_limit(Some(0)), 1);
    }

    #[test]
    fn clamp_limit_passes_through_valid() {
        assert_eq!(clamp_limit(Some(50)), 50);
    }

    #[test]
    fn snippet_short_text_unchanged() {
        assert_eq!(snippet("hello world", 200), "hello world");
    }

    #[test]
    fn snippet_truncates_at_word_boundary() {
        let long_text = "a ".repeat(150);
        let result = snippet(&long_text, 200);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 210); // 200 + "..."
    }

    #[test]
    fn estimate_tokens_rough_accuracy() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("abc"), 1); // 3 chars rounds up to 1 token
    }

    #[test]
    fn check_input_size_accepts_small() {
        assert!(check_input_size("hello", "field").is_ok());
    }

    #[test]
    fn check_input_size_rejects_large() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(check_input_size(&big, "body").is_err());
    }

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
