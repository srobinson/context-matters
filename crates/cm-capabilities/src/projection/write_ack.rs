//! `cx_*` write-tool ack YAML-text formatters (`store`, `update`, `deposit`,
//! `forget`).
//!
//! Consumed by the write-tool wire-swap sub (ALP-1737) to replace the
//! `serde_json!` blobs built in `crates/cm-cli/src/mcp/tools/{store,
//! update, deposit, forget}.rs` with compact, agent-legible YAML envelopes.
//! The target shapes live in
//! `research/cx-response-payload-redesign-context-matters.md` §5.3.
//!
//! Unlike the read-tool formatters in the sibling view modules, write acks
//! surface a small fixed set of identifiers and counters, not paginated
//! rows, and carry no relative-age columns. They take no reference `now`
//! and have no `_at` deterministic variant, mirroring
//! [`format_stats_view`](super::format_stats_view).
//!
//! A separate [`ForgetError`] struct surfaces per-row forget failures so
//! the formatter can enumerate them under an indented `errors:` block
//! without collapsing the id-to-reason mapping into a single opaque
//! `message` string.

use std::fmt::Write as _;

use super::{fmt_with_commas, hex_prefix};

/// Byte-prefix width used to slice the BLAKE3 content hash for display.
/// Eight hex chars is enough to catch routine dedup collisions during
/// manual debugging without dragging the full 64-char digest into the
/// wire payload; callers that need the full hash can fetch the entry
/// via `cx_get`.
const CONTENT_HASH_WIDTH: usize = 8;

/// One failed row from `cx_forget`, used by [`format_forget_ack`] to surface
/// the id-to-reason mapping under an indented `errors:` block.
///
/// The existing handler in `crates/cm-cli/src/mcp/tools/forget.rs` folds
/// per-row errors into an opaque `message` string, losing which ids
/// failed and why. This struct gives the formatter the structured input
/// it needs to enumerate failures explicitly.
#[derive(Debug, Clone)]
pub struct ForgetError {
    /// Full hyphenated UUID of the entry that failed to be forgotten.
    pub id: String,
    /// One-line reason for the failure. Rendered verbatim; callers must
    /// strip newlines before passing.
    pub error: String,
}

/// Render a `cx_store` ack as YAML text. The `superseded` parameter carries
/// the prior entry's id when this write supersedes an existing entry;
/// pass `None` for a fresh insert.
///
/// Field order is pinned so snapshot diffs stay readable: `stored`, then
/// optional `superseded`, then `scope`, `kind`, `content_hash`.
pub fn format_store_ack(
    id: &str,
    scope: &str,
    kind: &str,
    hash: &str,
    superseded: Option<&str>,
) -> String {
    let mut out = String::with_capacity(192);
    out.push_str("---\n");
    let _ = writeln!(out, "stored: {id}");
    if let Some(prev) = superseded {
        let _ = writeln!(out, "superseded: {prev}");
    }
    let _ = writeln!(out, "scope: {scope}");
    let _ = writeln!(out, "kind: {kind}");
    let _ = writeln!(
        out,
        "content_hash: {}",
        hex_prefix(hash, CONTENT_HASH_WIDTH)
    );
    out
}

/// Render a `cx_update` ack as YAML text. Minimal shape: just the updated
/// id and the new content-hash prefix. The scope and kind never change
/// under update semantics, so they are omitted from the envelope.
pub fn format_update_ack(id: &str, hash: &str) -> String {
    let mut out = String::with_capacity(96);
    out.push_str("---\n");
    let _ = writeln!(out, "updated: {id}");
    let _ = writeln!(
        out,
        "content_hash: {}",
        hex_prefix(hash, CONTENT_HASH_WIDTH)
    );
    out
}

/// Render a `cx_deposit` ack as YAML text. Branches on `summary_id`:
///
/// - `Some(id)` renders the summary's full id and a trailing `cx_get(...)`
///   hint comment, suppressing the per-entry id list since the caller can
///   read the summary to rehydrate individual rows.
/// - `None` renders an inline compact list of full UUIDs for every
///   deposited entry, so the caller can see at a glance which rows landed.
///
/// The `deposited:` counter pluralises `exchange` based on `entry_ids.len()`.
pub fn format_deposit_ack(entry_ids: &[String], summary_id: Option<&str>, scope: &str) -> String {
    let mut out = String::with_capacity(256);
    out.push_str("---\n");
    let count = entry_ids.len();
    let suffix = if count == 1 { "" } else { "s" };
    let _ = writeln!(out, "deposited: {count} exchange{suffix}");
    if let Some(summary) = summary_id {
        let _ = writeln!(out, "summary: {summary}");
        let _ = writeln!(out, "scope: {scope}");
        let _ = writeln!(out, "# cx_get(id=\"{summary}\") to read summary");
    } else {
        let ids: Vec<&str> = entry_ids.iter().map(String::as_str).collect();
        let _ = writeln!(out, "entry_ids: [{}]", ids.join(", "));
        let _ = writeln!(out, "scope: {scope}");
    }
    out
}

/// Render a `cx_forget` ack as YAML text. Surfaces the three disposition
/// counters unconditionally. When any rows failed, an indented `errors:`
/// block enumerates them id-by-id; otherwise a trailing advisory comment
/// confirms the happy path so callers do not have to parse for errors.
pub fn format_forget_ack(
    forgotten: u32,
    already_inactive: u32,
    not_found: u32,
    errors: &[ForgetError],
) -> String {
    let mut out = String::with_capacity(192);
    out.push_str("---\n");
    let _ = writeln!(out, "forgotten: {}", fmt_with_commas(forgotten));
    let _ = writeln!(
        out,
        "already_inactive: {}",
        fmt_with_commas(already_inactive)
    );
    let _ = writeln!(out, "not_found: {}", fmt_with_commas(not_found));
    if errors.is_empty() {
        let _ = writeln!(
            out,
            "# all requested ids handled, no further action required"
        );
    } else {
        let _ = writeln!(out, "errors: {}", fmt_with_commas(errors.len() as u32));
        for err in errors {
            let _ = writeln!(out, "  - id: {}  error: {}", err.id, err.error);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guard: deposit with one entry renders singular `exchange`, never
    /// plural. Covers the `count == 1` branch of the suffix selector
    /// that the snapshot fixtures miss because they use N != 1.
    #[test]
    fn deposit_ack_singular_entry_uses_no_plural() {
        let out = format_deposit_ack(
            &["019d8a01-0000-7000-8abc-000000000001".to_owned()],
            None,
            "global",
        );
        assert!(out.contains("deposited: 1 exchange\n"));
        assert!(!out.contains("exchanges"));
    }

    /// Guard: four-digit-plus counters must carry thousand separators,
    /// to avoid regressing the `fmt_with_commas` import after a future
    /// refactor.
    #[test]
    fn forget_ack_large_counters_are_comma_formatted() {
        let out = format_forget_ack(12_345, 6_789, 0, &[]);
        assert!(out.contains("forgotten: 12,345\n"));
        assert!(out.contains("already_inactive: 6,789\n"));
        // Three-digit values do not grow a comma.
        assert!(out.contains("not_found: 0\n"));
    }

    /// Guard: content_hash slicing is a byte prefix, not a char count,
    /// so a 64-char hex digest truncates to exactly 8 bytes and the
    /// remaining 56 bytes never leak into the envelope.
    #[test]
    fn store_ack_content_hash_prefix_is_eight_bytes() {
        let out = format_store_ack(
            "019d8a01-0000-7000-8abc-000000000000",
            "global",
            "decision",
            "b4c2a9de0f1e2a3b4c5d6e7f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f0a1b",
            None,
        );
        assert!(out.contains("content_hash: b4c2a9de\n"));
        assert!(!out.contains("b4c2a9de0f1e"));
    }
}
