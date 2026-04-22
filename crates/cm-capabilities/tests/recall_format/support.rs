use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::projection::RecallRow;
use cm_core::{Entry, EntryKind, EntryMeta, ScopePath};

pub(crate) fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

/// Derives a unique 64 char hex `content_hash` from the test row's
/// `id_hex` so every fixture row hashes differently by default. Keeps
/// the intra response dedup pass from flagging unrelated test rows as
/// dupes just because they all share a placeholder hash. Tests that
/// intentionally exercise the dedup codepath override this by calling
/// [`make_row_with_hash`] directly.
fn content_hash_from(id_hex: &str) -> String {
    let clean = id_hex.replace('-', "");
    assert!(
        clean.len() <= 64,
        "test fixture id_hex must fit inside 64 hex chars",
    );
    format!("{clean:0<64}")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_row(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
    score: Option<f32>,
) -> RecallRow {
    make_row_with_hash(
        id_hex,
        kind,
        title,
        body,
        scope,
        tags,
        updated_at,
        score,
        &content_hash_from(id_hex),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_row_with_hash(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
    score: Option<f32>,
    content_hash: &str,
) -> RecallRow {
    RecallRow {
        entry: Entry {
            id: Uuid::parse_str(id_hex).expect("test fixture uuid parses"),
            scope_path: ScopePath::parse(scope).expect("test fixture scope parses"),
            kind,
            title: title.to_owned(),
            body: body.to_owned(),
            content_hash: content_hash.to_owned(),
            meta: Some(EntryMeta {
                tags: tags.iter().map(|t| (*t).to_owned()).collect(),
                ..Default::default()
            }),
            created_by: "agent:claude-code".to_owned(),
            created_at: updated_at,
            updated_at,
            superseded_by: None,
        },
        score,
    }
}
