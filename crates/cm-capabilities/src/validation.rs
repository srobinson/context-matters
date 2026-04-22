use std::collections::HashMap;

use cm_core::{Confidence, EntryKind, EntryMeta};
use serde::Deserialize;
use uuid::Uuid;

use crate::constants::{DEFAULT_LIMIT, MAX_BATCH_IDS, MAX_INPUT_BYTES, MAX_LIMIT};
use crate::stats::TagSort;

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

/// Parse an entry kind string to the EntryKind enum.
pub fn parse_kind(s: &str) -> Result<EntryKind, String> {
    s.parse::<EntryKind>()
        .map_err(crate::error::cm_err_to_string)
}

/// Parse a stats tag-sort string to the TagSort enum.
pub fn parse_tag_sort(s: &str) -> Result<TagSort, String> {
    match s {
        "name" => Ok(TagSort::Name),
        "count" => Ok(TagSort::Count),
        other => Err(format!(
            "Invalid tag_sort '{other}'. Valid values: name, count."
        )),
    }
}

/// Parsed form of a caller-provided UUID batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUuidBatch {
    /// UUIDs in caller order, ready for store calls.
    pub uuids: Vec<Uuid>,
    /// Canonical lowercase hyphenated string form for projection diffs.
    pub canonical_ids: Vec<String>,
}

/// Parse one user-provided UUID into canonical typed form.
pub fn parse_uuid(raw: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw).map_err(|e| format!("invalid UUID '{raw}': {e}"))
}

/// Validate and parse an entry-id batch.
///
/// The projection layer compares requested IDs to `Entry::id.to_string()`
/// when computing missing rows, so this helper returns both typed UUIDs
/// and canonical strings in the same order.
pub fn parse_uuid_batch(ids: &[String]) -> Result<ParsedUuidBatch, String> {
    if ids.is_empty() {
        return Err("ids cannot be empty".to_owned());
    }
    if ids.len() > MAX_BATCH_IDS {
        return Err(format!("maximum {MAX_BATCH_IDS} ids per request"));
    }

    let mut uuids = Vec::with_capacity(ids.len());
    let mut canonical_ids = Vec::with_capacity(ids.len());
    for raw in ids {
        let id = parse_uuid(raw)?;
        uuids.push(id);
        canonical_ids.push(id.to_string());
    }

    Ok(ParsedUuidBatch {
        uuids,
        canonical_ids,
    })
}

/// JSON-deserialisable input shape for the `meta` blob on `cx_update` and
/// `cm update --meta`. Mirrors [`EntryMeta`] but accepts string-encoded
/// `confidence` and ISO 8601 `expires_at` that need to be parsed.
///
/// Lives in `cm-capabilities` so both the MCP handler and the CLI share a
/// single source of truth for the `--meta`/`meta` wire shape. Callers convert
/// to [`EntryMeta`] via [`MetaInput::into_entry_meta`], which validates the
/// string fields and returns a typed error on failure.
#[derive(Debug, Deserialize)]
pub struct MetaInput {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
}

impl MetaInput {
    /// Validate and project the wire-shape into [`EntryMeta`].
    ///
    /// Errors:
    /// - `confidence` must be one of `high`, `medium`, `low`.
    /// - `expires_at` must parse as RFC 3339 (ISO 8601).
    ///
    /// `extra` is always initialised to an empty map; the wire schema does
    /// not carry arbitrary key/value extensions today.
    pub fn into_entry_meta(self) -> Result<EntryMeta, String> {
        let confidence = match self.confidence.as_deref() {
            Some(c) => Some(parse_confidence(c)?),
            None => None,
        };
        let expires_at = match self.expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| format!("Invalid expires_at: {e}. Expected ISO 8601 format."))?,
            ),
            None => None,
        };
        Ok(EntryMeta {
            tags: self.tags,
            confidence,
            source: self.source,
            expires_at,
            priority: self.priority,
            extra: HashMap::new(),
        })
    }
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
    fn check_input_size_accepts_small() {
        assert!(check_input_size("hello", "field").is_ok());
    }

    #[test]
    fn check_input_size_rejects_large() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(check_input_size(&big, "body").is_err());
    }

    #[test]
    fn parse_confidence_valid() {
        assert_eq!(parse_confidence("high").unwrap(), Confidence::High);
        assert_eq!(parse_confidence("medium").unwrap(), Confidence::Medium);
        assert_eq!(parse_confidence("low").unwrap(), Confidence::Low);
    }

    #[test]
    fn parse_confidence_invalid() {
        assert!(parse_confidence("unknown").is_err());
    }

    #[test]
    fn parse_kind_valid() {
        assert_eq!(parse_kind("fact").unwrap(), EntryKind::Fact);
        assert_eq!(parse_kind("decision").unwrap(), EntryKind::Decision);
        assert_eq!(parse_kind("preference").unwrap(), EntryKind::Preference);
        assert_eq!(parse_kind("lesson").unwrap(), EntryKind::Lesson);
        assert_eq!(parse_kind("reference").unwrap(), EntryKind::Reference);
        assert_eq!(parse_kind("feedback").unwrap(), EntryKind::Feedback);
        assert_eq!(parse_kind("pattern").unwrap(), EntryKind::Pattern);
        assert_eq!(parse_kind("observation").unwrap(), EntryKind::Observation);
    }

    #[test]
    fn parse_kind_invalid_has_canonical_values() {
        let err = parse_kind("memo").unwrap_err();
        assert_eq!(
            err,
            "Invalid kind 'memo'. Valid values: fact, decision, preference, lesson, reference, feedback, pattern, observation."
        );
    }

    #[test]
    fn parse_tag_sort_valid() {
        assert_eq!(parse_tag_sort("name").unwrap(), TagSort::Name);
        assert_eq!(parse_tag_sort("count").unwrap(), TagSort::Count);
    }

    #[test]
    fn parse_tag_sort_invalid_has_canonical_values() {
        let err = parse_tag_sort("recent").unwrap_err();
        assert_eq!(err, "Invalid tag_sort 'recent'. Valid values: name, count.");
    }

    #[test]
    fn parse_uuid_batch_rejects_empty_ids() {
        let err = parse_uuid_batch(&[]).unwrap_err();
        assert_eq!(err, "ids cannot be empty");
    }

    #[test]
    fn parse_uuid_batch_rejects_too_many_ids() {
        let ids = vec!["019d8a01-0000-7000-8000-000000000001".to_owned(); MAX_BATCH_IDS + 1];
        let err = parse_uuid_batch(&ids).unwrap_err();
        assert_eq!(err, format!("maximum {MAX_BATCH_IDS} ids per request"));
    }

    #[test]
    fn parse_uuid_batch_rejects_invalid_uuid() {
        let err = parse_uuid_batch(&["not-a-uuid".to_owned()]).unwrap_err();
        assert!(err.contains("invalid UUID 'not-a-uuid'"));
    }

    #[test]
    fn parse_uuid_batch_returns_canonical_ids() {
        let ids = vec!["019D8A01000070008000000000000001".to_owned()];
        let parsed = parse_uuid_batch(&ids).unwrap();
        assert_eq!(parsed.uuids.len(), 1);
        assert_eq!(
            parsed.canonical_ids,
            vec!["019d8a01-0000-7000-8000-000000000001".to_owned()]
        );
    }

    /// Guard: `MetaInput` round-trips an empty JSON object into an
    /// `EntryMeta` with no fields set. `#[serde(default)]` on every field
    /// means `{}` must parse without complaining about missing keys.
    #[test]
    fn meta_input_empty_object_round_trips() {
        let input: MetaInput = serde_json::from_str("{}").unwrap();
        let meta = input.into_entry_meta().unwrap();
        assert!(meta.tags.is_empty());
        assert!(meta.confidence.is_none());
        assert!(meta.source.is_none());
        assert!(meta.expires_at.is_none());
        assert!(meta.priority.is_none());
        assert!(meta.extra.is_empty());
    }

    /// Guard: a fully populated `MetaInput` projects every field into the
    /// matching `EntryMeta` field with string confidence parsed to the enum
    /// and RFC 3339 `expires_at` parsed to a `DateTime<Utc>`.
    #[test]
    fn meta_input_full_object_parses_all_fields() {
        let raw = r#"{
            "tags": ["rust", "cli"],
            "confidence": "high",
            "source": "stuart",
            "expires_at": "2027-01-15T12:00:00Z",
            "priority": 7
        }"#;
        let input: MetaInput = serde_json::from_str(raw).unwrap();
        let meta = input.into_entry_meta().unwrap();
        assert_eq!(meta.tags, vec!["rust".to_owned(), "cli".to_owned()]);
        assert_eq!(meta.confidence, Some(Confidence::High));
        assert_eq!(meta.source.as_deref(), Some("stuart"));
        assert!(meta.expires_at.is_some());
        assert_eq!(meta.priority, Some(7));
    }

    #[test]
    fn meta_input_invalid_confidence_errors() {
        let input = MetaInput {
            tags: vec![],
            confidence: Some("maybe".to_owned()),
            source: None,
            expires_at: None,
            priority: None,
        };
        let err = input.into_entry_meta().unwrap_err();
        assert!(err.contains("Invalid confidence"));
    }

    #[test]
    fn meta_input_invalid_expires_at_errors() {
        let input = MetaInput {
            tags: vec![],
            confidence: None,
            source: None,
            expires_at: Some("not-a-date".to_owned()),
            priority: None,
        };
        let err = input.into_entry_meta().unwrap_err();
        assert!(err.contains("Invalid expires_at"));
    }
}
