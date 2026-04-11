//! Shared fixtures and YAML-envelope helpers for the cm-cli test binaries.
//!
//! Each `tests/*.rs` file is compiled as a separate binary, so helpers
//! that more than one binary needs live here to avoid the DRY violation
//! that otherwise forces each binary to carry its own copy. Every item
//! is marked `#[allow(dead_code)]` because individual test binaries only
//! import a subset, and Rust would otherwise flag the unused rest.

#![allow(dead_code)]

use cm_core::{ContextStore, MutationSource, NewScope, ScopePath, WriteContext};
use cm_store::{CmStore, schema};

/// Create an isolated store backed by a temp-file SQLite database.
///
/// The returned `TempDir` must stay alive for the duration of the test;
/// dropping it deletes the backing file out from under the pools.
pub async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();

    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

/// Create the `global` root scope in the store.
pub async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Global".to_owned(),
                meta: None,
            },
            &WriteContext::new(MutationSource::Mcp),
        )
        .await
        .unwrap();
}

/// Count rendered row lines in a `cx_browse` or `cx_recall` YAML envelope.
///
/// Row lines start with `"  - "` (two-space list indent + dash + space),
/// the one place where the view formatters emit entries. Header keys
/// (`total:`, `returned:`, etc.) and continuation/comment lines indent
/// further, so a strict prefix match is enough.
pub fn count_row_lines(text: &str) -> usize {
    text.lines().filter(|l| l.starts_with("  - ")).count()
}

/// Extract a `cx_browse` cursor from the pagination-trailer comment.
///
/// The formatter emits `# N more - cx_browse(cursor="XYZ", limit=L) to page`
/// at the end of the body when more pages exist. Returns `None` when the
/// trailer is absent or the cursor cannot be located.
pub fn extract_browse_cursor(text: &str) -> Option<String> {
    let line = text.lines().find(|l| l.contains("cx_browse(cursor="))?;
    let start = line.find("cursor=\"")? + "cursor=\"".len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// Extract the full uuid value from a `cx_store` ack YAML envelope.
///
/// `format_store_ack` emits `stored: <full-uuid>` on the second line of
/// the envelope (after the `---` marker). Downstream tests need the id
/// to chain subsequent tool calls against the entry they just created.
/// Panics if the marker is absent, since every passing cx_store call
/// must include it.
pub fn extract_stored_id(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| l.starts_with("stored: "))
        .expect("cx_store ack must contain `stored: <uuid>` line");
    line["stored: ".len()..].trim().to_owned()
}
