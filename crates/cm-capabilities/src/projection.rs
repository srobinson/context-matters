use chrono::{DateTime, Utc};
use cm_core::{Confidence, Entry, EntryMeta};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::constants::SNIPPET_LENGTH;

// ── Text Helpers ─────────────────────────────────────────────────

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

/// Check whether an entry has any of the specified tags.
pub fn entry_has_any_tag(entry: &Entry, tags: &[String]) -> bool {
    match &entry.meta {
        Some(meta) => meta.tags.iter().any(|t| tags.contains(t)),
        None => false,
    }
}

// ── Typed View Structs ───────────────────────────────────────────

/// Two-phase recall view: snippet instead of full body, minimal metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecallEntryView {
    pub id: String,
    pub scope_path: String,
    pub kind: String,
    pub title: String,
    pub snippet: String,
    pub created_by: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
}

/// Two-phase browse view: snippet instead of full body, includes timestamps and superseded_by.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowseEntryView {
    pub id: String,
    pub scope_path: String,
    pub kind: String,
    pub title: String,
    pub snippet: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Full entry view: includes body, content_hash, and all metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullEntryView {
    pub id: String,
    pub scope_path: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub content_hash: String,
    pub meta: Option<EntryMeta>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
}

// ── Mappers ──────────────────────────────────────────────────────

fn format_time(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

fn format_uuid(id: &Uuid) -> String {
    id.to_string()
}

fn extract_tags(meta: &Option<EntryMeta>) -> Vec<String> {
    match meta {
        Some(m) if !m.tags.is_empty() => m.tags.clone(),
        _ => Vec::new(),
    }
}

fn extract_confidence(meta: &Option<EntryMeta>) -> Option<Confidence> {
    meta.as_ref().and_then(|m| m.confidence)
}

/// Project an `Entry` into a `RecallEntryView` for two-phase recall responses.
pub fn project_recall_entry(entry: &Entry) -> RecallEntryView {
    RecallEntryView {
        id: format_uuid(&entry.id),
        scope_path: entry.scope_path.as_str().to_owned(),
        kind: entry.kind.as_str().to_owned(),
        title: entry.title.clone(),
        snippet: snippet(&entry.body, SNIPPET_LENGTH),
        created_by: entry.created_by.clone(),
        updated_at: format_time(&entry.updated_at),
        tags: extract_tags(&entry.meta),
        confidence: extract_confidence(&entry.meta),
    }
}

/// Project an `Entry` into a `BrowseEntryView` for two-phase browse responses.
pub fn project_browse_entry(entry: &Entry) -> BrowseEntryView {
    BrowseEntryView {
        id: format_uuid(&entry.id),
        scope_path: entry.scope_path.as_str().to_owned(),
        kind: entry.kind.as_str().to_owned(),
        title: entry.title.clone(),
        snippet: snippet(&entry.body, SNIPPET_LENGTH),
        created_by: entry.created_by.clone(),
        created_at: format_time(&entry.created_at),
        updated_at: format_time(&entry.updated_at),
        superseded_by: entry.superseded_by.map(|id| format_uuid(&id)),
        tags: extract_tags(&entry.meta),
    }
}

/// Project an `Entry` into a `FullEntryView` with complete content.
pub fn project_full_entry(entry: &Entry) -> FullEntryView {
    FullEntryView {
        id: format_uuid(&entry.id),
        scope_path: entry.scope_path.as_str().to_owned(),
        kind: entry.kind.as_str().to_owned(),
        title: entry.title.clone(),
        body: entry.body.clone(),
        content_hash: entry.content_hash.clone(),
        meta: entry.meta.clone(),
        created_by: entry.created_by.clone(),
        created_at: format_time(&entry.created_at),
        updated_at: format_time(&entry.updated_at),
        superseded_by: entry.superseded_by.map(|id| format_uuid(&id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_short_text_unchanged() {
        assert_eq!(snippet("hello world", 200), "hello world");
    }

    #[test]
    fn snippet_truncates_at_word_boundary() {
        let long_text = "a ".repeat(150);
        let result = snippet(&long_text, 200);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 210);
    }

    #[test]
    fn estimate_tokens_rough_accuracy() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("abc"), 1);
    }
}
