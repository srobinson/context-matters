use cm_core::CmError;

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
