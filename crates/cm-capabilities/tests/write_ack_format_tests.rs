//! Snapshot tests for `cm_capabilities::projection::write_ack` formatters.
//!
//! Covers the seven target shapes from
//! `research/cx-response-payload-redesign-context-matters.md` §5.3:
//!
//! - `store_ack_without_supersede` / `store_ack_with_supersede`
//! - `update_ack_minimal`
//! - `deposit_ack_with_summary` / `deposit_ack_without_summary_lists_ids`
//! - `forget_ack_happy_path_omits_details` / `forget_ack_with_errors_lists_them`
//!
//! Each test asserts byte-for-byte against a golden file on disk. The
//! supporting edge-case tests in `projection::write_ack::tests` already
//! cover singular/plural pluralisation, comma-formatted counters, and the
//! 8-byte content-hash prefix; this integration test guards the end-to-end
//! wire shape.
//!
//! If any target shape needs to change intentionally, update the relevant
//! golden under `tests/snapshots/write_ack_*.txt` alongside the source
//! change.

use cm_capabilities::projection::{
    ForgetError, format_deposit_ack, format_forget_ack, format_store_ack, format_update_ack,
};

const GOLDEN_STORE: &str = include_str!("snapshots/write_ack_store.txt");
const GOLDEN_STORE_SUPERSEDE: &str = include_str!("snapshots/write_ack_store_supersede.txt");
const GOLDEN_UPDATE: &str = include_str!("snapshots/write_ack_update.txt");
const GOLDEN_DEPOSIT_SUMMARY: &str = include_str!("snapshots/write_ack_deposit_summary.txt");
const GOLDEN_DEPOSIT_IDS: &str = include_str!("snapshots/write_ack_deposit_ids.txt");
const GOLDEN_FORGET_HAPPY: &str = include_str!("snapshots/write_ack_forget_happy.txt");
const GOLDEN_FORGET_ERRORS: &str = include_str!("snapshots/write_ack_forget_errors.txt");

/// Deterministic UUIDs used across the snapshot fixtures. Hand-crafted so
/// each per-entry id is clearly distinct at its first 8-char prefix, which
/// lets the inline deposit id list render as five unique short ids
/// without triggering any collision auto-extend logic.
const STORED_ID: &str = "019d8a01-9c4f-7891-8abc-000000000000";
const SUPERSEDED_ID: &str = "019d7f3e-0000-7000-8abc-000000000000";
const SUMMARY_ID: &str = "019d8b01-0000-7000-8abc-000000000001";

/// 64-char BLAKE3 hex digest. The formatter only renders the first 8
/// chars; the trailing bytes are decorative but kept realistic so future
/// readers can see the canonical input shape.
const CONTENT_HASH: &str = "b4c2a9de0f1e2a3b4c5d6e7f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f0a1b";
const UPDATE_HASH: &str = "c5d3b8ef1a2b3c4d5e6f70819a2b3c4d5e6f70819a2b3c4d5e6f70819a2b3c4d";

fn deposit_entry_ids() -> Vec<String> {
    vec![
        "019d8a01-0000-7000-8abc-000000000001".to_owned(),
        "019d8a02-0000-7000-8abc-000000000002".to_owned(),
        "019d8a03-0000-7000-8abc-000000000003".to_owned(),
        "019d8a04-0000-7000-8abc-000000000004".to_owned(),
        "019d8a05-0000-7000-8abc-000000000005".to_owned(),
    ]
}

#[test]
fn store_ack_without_supersede() {
    let rendered = format_store_ack(
        STORED_ID,
        "global/project:helioy",
        "decision",
        CONTENT_HASH,
        None,
    );
    assert_eq!(
        rendered, GOLDEN_STORE,
        "store_ack_without_supersede rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn store_ack_with_supersede() {
    let rendered = format_store_ack(
        STORED_ID,
        "global/project:helioy",
        "decision",
        CONTENT_HASH,
        Some(SUPERSEDED_ID),
    );
    assert_eq!(
        rendered, GOLDEN_STORE_SUPERSEDE,
        "store_ack_with_supersede rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn update_ack_minimal() {
    let rendered = format_update_ack(STORED_ID, UPDATE_HASH);
    assert_eq!(
        rendered, GOLDEN_UPDATE,
        "update_ack_minimal rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn deposit_ack_with_summary() {
    let rendered = format_deposit_ack(&deposit_entry_ids(), Some(SUMMARY_ID), "global");
    assert_eq!(
        rendered, GOLDEN_DEPOSIT_SUMMARY,
        "deposit_ack_with_summary rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn deposit_ack_without_summary_lists_ids() {
    let rendered = format_deposit_ack(&deposit_entry_ids(), None, "global");
    assert_eq!(
        rendered, GOLDEN_DEPOSIT_IDS,
        "deposit_ack_without_summary_lists_ids rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn forget_ack_happy_path_omits_details() {
    let rendered = format_forget_ack(3, 1, 0, &[]);
    assert_eq!(
        rendered, GOLDEN_FORGET_HAPPY,
        "forget_ack_happy_path_omits_details rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

#[test]
fn forget_ack_with_errors_lists_them() {
    let errors = vec![ForgetError {
        id: "019d8a99-0000-7000-8abc-000000000000".to_owned(),
        error: "write conflict".to_owned(),
    }];
    let rendered = format_forget_ack(2, 0, 0, &errors);
    assert_eq!(
        rendered, GOLDEN_FORGET_ERRORS,
        "forget_ack_with_errors_lists_them rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}
