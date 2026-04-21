//! Sort-aware opaque cursor encoding/decoding for keyset pagination.
//!
//! Each `BrowseSort` variant encodes its own cursor payload containing
//! the last-seen values for all ORDER BY columns. Decoding validates
//! that the cursor's sort mode matches the current query.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use cm_core::{BrowseSort, CmError, Entry};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite};

/// Internal cursor payload. Encodes the sort mode and keyset values.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CursorPayload {
    /// Sort mode that produced this cursor (validated on decode).
    sort: BrowseSort,
    /// Primary sort column value for text sorts (title, scope_path, kind).
    /// None for time-based sorts (Recent, Oldest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    val: Option<String>,
    /// `updated_at` timestamp (secondary sort for text sorts, primary for time sorts).
    ts: DateTime<Utc>,
    /// Entry ID (final tiebreaker in all sort modes).
    id: uuid::Uuid,
}

/// Encode a cursor from the last entry on the current page.
pub(crate) fn encode_cursor(entry: &Entry, sort: BrowseSort) -> String {
    let val = match sort {
        BrowseSort::Recent | BrowseSort::Oldest => None,
        BrowseSort::TitleAsc | BrowseSort::TitleDesc => Some(entry.title.clone()),
        BrowseSort::ScopeAsc | BrowseSort::ScopeDesc => Some(entry.scope_path.as_str().to_owned()),
        BrowseSort::KindAsc | BrowseSort::KindDesc => Some(entry.kind.as_str().to_owned()),
    };

    let payload = CursorPayload {
        sort,
        val,
        ts: entry.updated_at,
        id: entry.id,
    };

    let json = serde_json::to_string(&payload).expect("cursor serialization");
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// Decoded cursor fields needed to build keyset WHERE clauses.
pub(crate) struct DecodedCursor {
    /// Primary sort column value (for text sorts).
    pub val: Option<String>,
    /// `updated_at` as ISO 8601 string for SQL binding.
    pub ts: String,
    /// Entry ID as string for SQL binding.
    pub id: String,
}

/// Decode an opaque cursor string and validate the sort mode matches.
pub(crate) fn decode_cursor(
    encoded: &str,
    expected_sort: BrowseSort,
) -> Result<DecodedCursor, CmError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| CmError::Validation("Invalid cursor format".into()))?;

    let payload: CursorPayload = serde_json::from_slice(&bytes)
        .map_err(|_| CmError::Validation("Invalid cursor format".into()))?;

    if payload.sort != expected_sort {
        return Err(CmError::Validation(format!(
            "Cursor sort mismatch: cursor is {:?} but query uses {:?}",
            payload.sort, expected_sort
        )));
    }

    Ok(DecodedCursor {
        val: payload.val,
        ts: payload.ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        id: payload.id.to_string(),
    })
}

/// SQL ORDER BY clause for a given sort mode.
pub(crate) fn order_by_clause(sort: BrowseSort) -> &'static str {
    match sort {
        BrowseSort::Recent => "ORDER BY updated_at DESC, id DESC",
        BrowseSort::Oldest => "ORDER BY updated_at ASC, id ASC",
        BrowseSort::TitleAsc => "ORDER BY title ASC, updated_at DESC, id DESC",
        BrowseSort::TitleDesc => "ORDER BY title DESC, updated_at DESC, id DESC",
        BrowseSort::ScopeAsc => "ORDER BY scope_path ASC, updated_at DESC, id DESC",
        BrowseSort::ScopeDesc => "ORDER BY scope_path DESC, updated_at DESC, id DESC",
        BrowseSort::KindAsc => "ORDER BY kind ASC, updated_at DESC, id DESC",
        BrowseSort::KindDesc => "ORDER BY kind DESC, updated_at DESC, id DESC",
    }
}

/// Append keyset pagination conditions and binds for the cursor.
///
/// The condition mirrors the ORDER BY clause to produce correct keyset pagination.
pub(crate) fn push_cursor_condition(
    query: &mut QueryBuilder<'_, Sqlite>,
    cursor: &DecodedCursor,
    sort: BrowseSort,
) {
    match sort {
        BrowseSort::Recent => {
            // ORDER BY updated_at DESC, id DESC
            push_time_cursor_condition(query, cursor, "<", "<");
        }
        BrowseSort::Oldest => {
            // ORDER BY updated_at ASC, id ASC
            push_time_cursor_condition(query, cursor, ">", ">");
        }
        BrowseSort::TitleAsc => {
            // ORDER BY title ASC, updated_at DESC, id DESC
            push_text_cursor_condition(query, "title", ">", cursor);
        }
        BrowseSort::TitleDesc => {
            // ORDER BY title DESC, updated_at DESC, id DESC
            push_text_cursor_condition(query, "title", "<", cursor);
        }
        BrowseSort::ScopeAsc => {
            push_text_cursor_condition(query, "scope_path", ">", cursor);
        }
        BrowseSort::ScopeDesc => {
            push_text_cursor_condition(query, "scope_path", "<", cursor);
        }
        BrowseSort::KindAsc => {
            push_text_cursor_condition(query, "kind", ">", cursor);
        }
        BrowseSort::KindDesc => {
            push_text_cursor_condition(query, "kind", "<", cursor);
        }
    }
}

fn push_time_cursor_condition(
    query: &mut QueryBuilder<'_, Sqlite>,
    cursor: &DecodedCursor,
    time_op: &'static str,
    id_op: &'static str,
) {
    query.push("(updated_at ");
    query.push(time_op);
    query.push(" ");
    query.push_bind(cursor.ts.clone());
    query.push(" OR (updated_at = ");
    query.push_bind(cursor.ts.clone());
    query.push(" AND id ");
    query.push(id_op);
    query.push(" ");
    query.push_bind(cursor.id.clone());
    query.push("))");
}

fn push_text_cursor_condition(
    query: &mut QueryBuilder<'_, Sqlite>,
    column: &'static str,
    text_op: &'static str,
    cursor: &DecodedCursor,
) {
    let val = cursor.val.as_deref().unwrap_or("");

    query.push("(");
    query.push(column);
    query.push(" ");
    query.push(text_op);
    query.push(" ");
    query.push_bind(val.to_owned());
    query.push(" OR (");
    query.push(column);
    query.push(" = ");
    query.push_bind(val.to_owned());
    query.push(" AND (updated_at < ");
    query.push_bind(cursor.ts.clone());
    query.push(" OR (updated_at = ");
    query.push_bind(cursor.ts.clone());
    query.push(" AND id < ");
    query.push_bind(cursor.id.clone());
    query.push("))))");
}
