//! Projection helpers and typed view structs for presenting `Entry` data in
//! two-phase recall/browse/get responses.
//!
//! Sub-modules:
//! - [`text`] — snippet generation, frontmatter/heading stripping, query matching.
//! - [`aggregation`] — short ids, relative age, histograms, uniform-key hoisting.

mod aggregation;
mod text;

pub use aggregation::*;
pub use text::*;

use chrono::{DateTime, Utc};
use cm_core::{Confidence, Entry, EntryMeta};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::constants::SNIPPET_LENGTH;

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
    /// Estimated tokens for the full entry body (not the snippet).
    pub token_estimate: u32,
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
        token_estimate: {
            let full = serde_json::to_string(&project_full_entry(entry)).unwrap_or_default();
            estimate_tokens(&full)
        },
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
