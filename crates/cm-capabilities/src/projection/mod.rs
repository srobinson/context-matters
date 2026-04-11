//! Projection helpers and typed view structs for presenting `Entry` data in
//! two-phase recall/browse/get responses.
//!
//! Sub-modules:
//! - [`text`] — snippet generation, frontmatter/heading stripping, query matching.
//! - [`aggregation`] — short ids, relative age, histograms, uniform-key hoisting.
//! - [`browse_view`] — YAML-text formatter for `BrowseResult` MCP responses.
//! - [`recall_view`] — YAML-text formatter for `RecallResult` MCP responses.
//! - [`get_view`] — YAML-text formatter for `cx_get` MCP responses.
//! - [`stats_view`] — YAML-text formatter for `cx_stats` MCP responses.

mod aggregation;
mod browse_view;
mod get_view;
mod recall_view;
mod stats_view;
mod text;

pub use aggregation::*;
pub use browse_view::*;
pub use get_view::*;
pub use recall_view::*;
pub use stats_view::*;
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

// ── Recall Row ───────────────────────────────────────────────────

/// An `Entry` paired with an optional FTS5 relevance score.
///
/// Produced by the recall pipeline. The `score` field carries the raw
/// BM25 value (negative float, lower = better) on the `Search` routing
/// branch and is `None` on every other branch (`TagScopeWalk`,
/// `ScopeResolve`, `BrowseFallback`) where no relevance ranking applies.
///
/// Normalisation to `0..=1` happens later, in the recall formatter, so
/// that scaling is per-query and per-slice rather than per-entry.
#[derive(Debug, Clone)]
pub struct RecallRow {
    pub entry: Entry,
    pub score: Option<f32>,
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
        // Estimate directly from the body byte length. Previously this field
        // serialised `project_full_entry(entry)` to JSON just to feed it to
        // `estimate_tokens`, which in turn divided bytes by 4 — two full
        // copies (entry → FullEntryView → String) per row for a result the
        // body alone answers to within a few percent.
        token_estimate: estimate_tokens(&entry.body),
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
    use cm_core::{EntryKind, ScopePath};

    fn entry_with_body(body: &str) -> Entry {
        Entry {
            id: uuid::Uuid::now_v7(),
            scope_path: ScopePath::global(),
            kind: EntryKind::Fact,
            title: "t".to_owned(),
            body: body.to_owned(),
            content_hash: "0".repeat(64),
            meta: None,
            created_by: "test".to_owned(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            superseded_by: None,
        }
    }

    /// `project_recall_entry.token_estimate` must track the raw body byte
    /// length (chars / 4) — nothing else. Previously it round-tripped
    /// `project_full_entry(entry)` through `serde_json::to_string`, which
    /// inflated the estimate by the serialized view overhead (id, hash,
    /// timestamps, field names) and re-allocated the body twice per row.
    ///
    /// The check is structural: for a 1000-byte body, the estimate must
    /// equal `estimate_tokens(body)`, not `estimate_tokens(serialized_view)`.
    #[test]
    fn recall_token_estimate_tracks_only_body_bytes() {
        let body = "a".repeat(1000);
        let entry = entry_with_body(&body);

        let view = project_recall_entry(&entry);

        // estimate_tokens is body.len().div_ceil(4) == 250
        assert_eq!(view.token_estimate, estimate_tokens(&body));
        assert_eq!(view.token_estimate, 250);

        // Sanity: serialising the full view would yield far more bytes than
        // the body alone, so if the old formula were still in place the
        // estimate would be noticeably larger.
        let full_view_bytes = serde_json::to_string(&project_full_entry(&entry))
            .unwrap()
            .len();
        assert!(
            full_view_bytes > body.len(),
            "full view serialisation should exceed raw body length"
        );
        assert!(
            view.token_estimate < estimate_tokens(&"a".repeat(full_view_bytes)),
            "token estimate must not reflect the serialised view size"
        );
    }

    #[test]
    fn recall_token_estimate_scales_linearly_with_body() {
        let small = project_recall_entry(&entry_with_body(&"x".repeat(400)));
        let large = project_recall_entry(&entry_with_body(&"x".repeat(4000)));

        // 10x body → 10x token estimate (within rounding).
        assert_eq!(small.token_estimate, 100);
        assert_eq!(large.token_estimate, 1000);
    }
}
