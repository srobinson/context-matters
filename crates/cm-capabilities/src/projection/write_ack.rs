//! `cx_*` write-tool ack receipts and YAML text formatters (`store`,
//! `update`, `deposit`, `forget`).
//!
//! Structured receipts back the MCP `structuredContent` payloads. YAML
//! formatters provide the parallel text channel for agents and older clients.
//!
//! Unlike the read-tool formatters in the sibling view modules, write acks
//! surface a small fixed set of identifiers and counters, not paginated
//! rows, and carry no relative-age columns. They take no reference `now`
//! and have no `_at` deterministic variant, mirroring
//! [`format_stats_view`](super::format_stats_view).
//!
//! The [`ForgetError`](crate::forget::ForgetError) struct that surfaces
//! per-row forget failures lives next to the `forget` capability; this
//! module imports it so [`format_forget_ack`] can render the indented
//! `errors:` block without a crate-level dependency inversion.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::{fmt_with_commas, hex_prefix};
use crate::{
    deposit::DepositResult,
    forget::{ForgetError, ForgetResult},
    store::StoreResult,
    update::UpdateResult,
};

/// Byte-prefix width used to slice the BLAKE3 content hash for display.
/// Eight hex chars is enough to catch routine dedup collisions during
/// manual debugging without dragging the full 64-char digest into the
/// wire payload; callers that need the full hash can fetch the entry
/// via `cx_get`.
const CONTENT_HASH_WIDTH: usize = 8;

/// Structured receipt emitted by `cx_store`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreReceipt {
    pub id: String,
    pub scope_path: String,
    pub kind: String,
    pub content_hash: String,
    pub superseded_id: Option<String>,
    pub scope_created: bool,
}

impl From<&StoreResult> for StoreReceipt {
    fn from(result: &StoreResult) -> Self {
        Self {
            id: result.entry_id.clone(),
            scope_path: result.scope_path.clone(),
            kind: result.kind.as_str().to_owned(),
            content_hash: result.content_hash.clone(),
            superseded_id: result.superseded_id.clone(),
            scope_created: result.scope_created,
        }
    }
}

pub fn project_store_receipt(result: &StoreResult) -> StoreReceipt {
    StoreReceipt::from(result)
}

/// Structured receipt emitted by `cx_deposit`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepositReceipt {
    pub deposited: usize,
    pub entry_ids: Vec<String>,
    pub summary_id: Option<String>,
    pub scope_path: String,
}

impl From<&DepositResult> for DepositReceipt {
    fn from(result: &DepositResult) -> Self {
        let entry_ids: Vec<String> = result.entry_ids.iter().map(ToString::to_string).collect();

        Self {
            deposited: entry_ids.len(),
            entry_ids,
            summary_id: result.summary_id.map(|id| id.to_string()),
            scope_path: result.scope_path.clone(),
        }
    }
}

pub fn project_deposit_receipt(result: &DepositResult) -> DepositReceipt {
    DepositReceipt::from(result)
}

/// Structured receipt emitted by `cx_update`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateReceipt {
    pub id: String,
    pub content_hash: String,
}

impl From<&UpdateResult> for UpdateReceipt {
    fn from(result: &UpdateResult) -> Self {
        Self {
            id: result.updated_id.clone(),
            content_hash: result.content_hash.clone(),
        }
    }
}

pub fn project_update_receipt(result: &UpdateResult) -> UpdateReceipt {
    UpdateReceipt::from(result)
}

/// One failed row in a `cx_forget` receipt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgetReceiptError {
    pub id: String,
    pub error: String,
}

impl From<&ForgetError> for ForgetReceiptError {
    fn from(error: &ForgetError) -> Self {
        Self {
            id: error.id.clone(),
            error: error.error.clone(),
        }
    }
}

/// Structured receipt emitted by `cx_forget`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgetReceipt {
    pub forgotten: u32,
    pub already_inactive: u32,
    pub not_found: u32,
    pub errors: Vec<ForgetReceiptError>,
}

impl From<&ForgetResult> for ForgetReceipt {
    fn from(result: &ForgetResult) -> Self {
        Self {
            forgotten: result.forgotten,
            already_inactive: result.already_inactive,
            not_found: result.not_found,
            errors: result.errors.iter().map(ForgetReceiptError::from).collect(),
        }
    }
}

pub fn project_forget_receipt(result: &ForgetResult) -> ForgetReceipt {
    ForgetReceipt::from(result)
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
