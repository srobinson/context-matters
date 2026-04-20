//! Forget capability: soft-delete entries by ID in batch.
//!
//! Lifts the per-row loop out of the `cx_forget` MCP handler and exposes a
//! channel-neutral [`forget`] function that both MCP (`cx_forget`) and the
//! CLI (`cm forget`) call with their own [`WriteContext`]. The function
//! returns a [`ForgetResult`] with disposition counts and per-row error
//! details; callers render it through
//! [`crate::projection::format_forget_ack`].
//!
//! Validation (non-empty list, ≤ `MAX_BATCH_IDS`, valid UUID format) lives
//! here rather than in each handler so both channels surface the same
//! errors byte-for-byte. Entry-level failures never short-circuit the
//! loop: a bad row records a [`ForgetError`] and the remaining IDs still
//! get their chance.

use cm_core::{CmError, ContextStore, WriteContext};
use uuid::Uuid;

use crate::constants::MAX_BATCH_IDS;

/// One failed row from a forget batch, surfacing the ID-to-reason mapping
/// for [`crate::projection::format_forget_ack`]'s indented `errors:` block.
///
/// Previously lived in `projection::write_ack` next to the formatter that
/// consumes it; relocated here because it is a capability-layer domain
/// type that the projection layer imports, not the other way round.
#[derive(Debug, Clone)]
pub struct ForgetError {
    /// Full hyphenated UUID of the entry that failed to be forgotten.
    pub id: String,
    /// One-line reason for the failure. Rendered verbatim; callers must
    /// strip newlines before passing.
    pub error: String,
}

/// Input for a forget operation.
#[derive(Debug, Clone)]
pub struct ForgetRequest {
    /// Full hyphenated UUIDs to soft-delete. Maximum `MAX_BATCH_IDS` per
    /// request; empty requests are rejected with a validation error so
    /// the caller sees an early failure rather than a silent no-op.
    pub ids: Vec<String>,
}

/// Result of a forget operation.
///
/// Every ID in the request lands in exactly one of the three counters
/// (or the `errors` list) so `forgotten + already_inactive + not_found +
/// errors.len() == request.ids.len()` always holds. Callers that want to
/// assert the happy path can check `errors.is_empty()`.
#[derive(Debug, Clone, Default)]
pub struct ForgetResult {
    /// Count of entries that were active and are now soft-deleted.
    pub forgotten: u32,
    /// Count of entries that were already soft-deleted before the call.
    pub already_inactive: u32,
    /// Count of IDs that did not resolve to any known entry.
    pub not_found: u32,
    /// Per-row failures with ID and reason.
    pub errors: Vec<ForgetError>,
}

/// Soft-delete each requested entry, recording the provenance carried by
/// `ctx`. Entries already marked `superseded_by` are counted as
/// `already_inactive` without a second write. Missing entries land in
/// `not_found`; any other storage error lands in `errors` with the full
/// error string.
///
/// Validates the request before touching the store so callers see input
/// errors (`empty`, `too many`, `invalid UUID`) as `CmError::Validation`
/// rather than partial results.
pub async fn forget(
    store: &impl ContextStore,
    request: ForgetRequest,
    ctx: &WriteContext,
) -> Result<ForgetResult, CmError> {
    if request.ids.is_empty() {
        return Err(CmError::Validation("ids cannot be empty".to_owned()));
    }
    if request.ids.len() > MAX_BATCH_IDS {
        return Err(CmError::Validation(format!(
            "maximum {MAX_BATCH_IDS} ids per request"
        )));
    }

    let mut uuids: Vec<Uuid> = Vec::with_capacity(request.ids.len());
    for raw in &request.ids {
        let id = Uuid::parse_str(raw)
            .map_err(|_| CmError::Validation(format!("invalid UUID format: '{raw}'")))?;
        uuids.push(id);
    }

    let mut result = ForgetResult::default();
    for &id in &uuids {
        match store.get_entry(id).await {
            Ok(entry) => {
                if entry.superseded_by.is_some() {
                    result.already_inactive += 1;
                } else {
                    match store.forget_entry(id, ctx).await {
                        Ok(()) => result.forgotten += 1,
                        Err(e) => result.errors.push(ForgetError {
                            id: id.to_string(),
                            error: format!("{e}"),
                        }),
                    }
                }
            }
            Err(CmError::EntryNotFound(_)) => {
                result.not_found += 1;
            }
            Err(e) => {
                result.errors.push(ForgetError {
                    id: id.to_string(),
                    error: format!("{e}"),
                });
            }
        }
    }

    Ok(result)
}
