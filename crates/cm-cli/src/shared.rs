//! Reusable helpers shared between cm-cli (MCP server) and cm-web (HTTP API).
//!
//! Most logic has migrated to cm-capabilities. This module re-exports for
//! backward compatibility and holds cm-cli-specific helpers.

use cm_core::{CmError, ContextStore, Entry, EntryFilter, EntryKind, Pagination, ScopePath};
use serde_json::Value;

// ── Re-exports from cm-capabilities ──────────────────────────────

pub use cm_capabilities::constants::{
    DEFAULT_LIMIT, MAX_BATCH_IDS, MAX_INPUT_BYTES, MAX_LIMIT, SNIPPET_LENGTH,
};
pub use cm_capabilities::error::cm_err_to_string;
pub use cm_capabilities::projection::{
    BrowseEntryView, FullEntryView, RecallEntryView, entry_has_any_tag, estimate_tokens,
    project_browse_entry, project_full_entry, project_recall_entry, snippet,
};
pub use cm_capabilities::scope::ensure_scope_chain;
pub use cm_capabilities::validation::{check_input_size, clamp_limit, parse_confidence};

// ── cm-cli-specific helpers ──────────────────────────────────────

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
