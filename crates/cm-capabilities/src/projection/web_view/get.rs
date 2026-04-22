use std::collections::HashSet;

use chrono::{DateTime, Utc};
use cm_core::Entry;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::super::get_view::confidence_as_str;
use super::super::relative_age;

/// Full-body row shape for a [`WebGetView`] response.
///
/// Mirrors the YAML `format_get_view` output: full UUID in `id`, full
/// body in `body`, scope/kind stringified, relative age, tags and
/// confidence when metadata is present. Structurally parallel to
/// `WebBrowseRow` and `WebRecallRow` so the frontend can reuse
/// row-rendering primitives across the three views.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebGetRow {
    pub id: String,
    pub title: String,
    pub scope: String,
    pub kind: String,
    pub age: String,
    pub body: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
}

/// Full projection of a `cx_get` response for the cm-web HTTP API and
/// the MCP 2025-06-18 `structuredContent` channel.
///
/// Structurally parallel to `format_get_view` output: `requested` and
/// `found` are counters, `missing` is the explicit diff of requested
/// IDs the store did not return, and `entries` carries the full-body
/// row list. `missing` is omitted when every requested ID was found;
/// `entries` is omitted when the store returned nothing.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebGetView {
    pub requested: usize,
    pub found: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<WebGetRow>,
}

/// Project store-returned entries and the raw requested-id list into a
/// [`WebGetView`].
///
/// Takes the same `(found, requested)` arity as `format_get_view` so
/// the two projections stay in lock-step on the missing-id diff. The
/// get view always carries the full UUID in `id` because the caller
/// already knows the ID it asked for and the row is keyed by it.
///
/// Captures `Utc::now()` once for relative-age formatting and
/// delegates to [`project_web_get_at`] so tests can pin the age column.
pub fn project_web_get(found: &[Entry], requested: &[String]) -> WebGetView {
    project_web_get_at(found, requested, Utc::now())
}

/// Deterministic variant of [`project_web_get`] that takes an explicit
/// reference `now` for relative-age rendering. Production callers
/// should prefer [`project_web_get`].
pub fn project_web_get_at(found: &[Entry], requested: &[String], now: DateTime<Utc>) -> WebGetView {
    let found_ids: HashSet<String> = found.iter().map(|e| e.id.to_string()).collect();
    // Preserve requested-id order so the frontend sees the same order
    // the caller asked for. The YAML view renders `missing:` in the
    // same order for the same reason.
    let missing: Vec<String> = requested
        .iter()
        .filter(|id| !found_ids.contains(id.as_str()))
        .cloned()
        .collect();

    let entries: Vec<WebGetRow> = found
        .iter()
        .map(|entry| {
            let tags = entry
                .meta
                .as_ref()
                .map(|m| m.tags.clone())
                .unwrap_or_default();
            let confidence = entry
                .meta
                .as_ref()
                .and_then(|m| m.confidence)
                .map(|c| confidence_as_str(c).to_owned());
            WebGetRow {
                id: entry.id.to_string(),
                title: entry.title.clone(),
                scope: entry.scope_path.as_str().to_owned(),
                kind: entry.kind.as_str().to_owned(),
                age: relative_age(entry.updated_at, now),
                body: entry.body.clone(),
                tags,
                confidence,
            }
        })
        .collect();

    WebGetView {
        requested: requested.len(),
        found: found.len(),
        missing,
        entries,
    }
}
