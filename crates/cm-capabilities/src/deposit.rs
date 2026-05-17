//! Deposit capability: batch-store conversation exchanges as observation
//! entries, optionally linked to a single session summary.
//!
//! Lifts the per-exchange loop, optional summary creation, and
//! `elaborates` relation writes out of the `cx_deposit` MCP handler and
//! exposes a channel-neutral [`deposit`] function that both MCP
//! (`cx_deposit`) and the CLI (`cm deposit`) call with their own
//! [`WriteContext`]. Returns a [`DepositResult`] with the created entry
//! IDs and optional summary ID; callers render it through
//! [`crate::projection::format_deposit_ack`].
//!
//! Validation (non-empty exchanges, ≤ [`MAX_EXCHANGES`], per-field byte
//! caps via [`check_input_size`], explicit-title length caps) lives here
//! rather than in each handler so both channels surface the same errors
//! byte-for-byte. Scope chain auto-creation piggybacks on
//! [`ensure_scope_chain`] so users never need to pre-create a scope before
//! depositing a session log.

use cm_core::{
    CmError, ContextStore, EntryKind, EntryMeta, NewEntry, RelationKind, ScopePath, WriteContext,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::projection::snippet;
use crate::scope::{ScopeSelector, ensure_scope_chain_with_status, resolve_scope_selection};
use crate::validation::check_input_size;

/// Maximum exchanges per deposit call. Kept low because each exchange
/// becomes one `NewEntry` write and, when a summary is provided, one
/// additional `create_relation` call. Users batching larger session logs
/// should split into multiple deposits.
pub const MAX_EXCHANGES: usize = 50;

/// Title truncation length for exchange entries. Matches the inline
/// [`snippet`] cap used by the MCP handler before this capability was
/// extracted; changing it affects the default title of every exchange
/// stored without an explicit `title` field.
pub const EXCHANGE_TITLE_LEN: usize = 80;

/// One conversation exchange. `title` is optional; when omitted, the
/// entry title is derived from the first [`EXCHANGE_TITLE_LEN`] bytes of
/// `user` via [`snippet`].
///
/// `Deserialize` is derived so MCP can parse this straight off the wire
/// and the CLI can parse it from a JSON blob passed via `--exchanges`
/// (with `-` sourcing the blob from stdin).
#[derive(Debug, Clone, Deserialize)]
pub struct Exchange {
    /// User-side text.
    pub user: String,
    /// Assistant-side text.
    pub assistant: String,
    /// Optional explicit title. When set, must be 1..=[`EXCHANGE_TITLE_LEN`]
    /// bytes; the capability rejects empty or over-long titles as a
    /// [`CmError::Validation`].
    #[serde(default)]
    pub title: Option<String>,
}

/// Input for a deposit operation.
#[derive(Debug, Clone)]
pub struct DepositRequest {
    /// Exchanges to store. Each becomes one [`EntryKind::Observation`]
    /// entry tagged `conversation`.
    pub exchanges: Vec<Exchange>,
    /// Optional summary. When provided, a second entry is created with
    /// the tags `conversation, summary` and linked to every exchange via
    /// an [`RelationKind::Elaborates`] relation.
    pub summary: Option<String>,
    /// Target scope selector. Defaults to `global` when unset.
    pub scope: Option<ScopeSelector>,
    /// Attribution string stamped on every created entry.
    pub created_by: String,
}

/// Result of a deposit operation.
///
/// `entry_ids` preserves the order of the incoming `exchanges`, so the
/// i-th id corresponds to the i-th exchange. `summary_id` is `Some` iff
/// the request carried a `summary` text. `scope_path` is the normalised
/// path string (post [`ScopePath::parse`]) suitable for the formatter.
#[derive(Debug, Clone)]
pub struct DepositResult {
    /// Created exchange entry IDs, in request order.
    pub entry_ids: Vec<Uuid>,
    /// Created summary entry ID, if `summary` was provided.
    pub summary_id: Option<Uuid>,
    /// Normalised scope path string, as returned by [`ScopePath::as_str`].
    pub scope_path: String,
}

/// Batch-store conversation exchanges, optionally linked to a session
/// summary.
///
/// Validates the request before touching the store so callers see input
/// errors (empty list, over-large batch, over-size field, invalid
/// explicit title) as [`CmError::Validation`] rather than partial writes.
/// Scope chain auto-creation runs before the exchange loop so that the
/// first entry can never be rejected for a missing scope.
///
/// Each exchange becomes one [`EntryKind::Observation`] with body
/// `"{user}\n\n---\n\n{assistant}"` and the tag `conversation`. When
/// `summary` is provided, an additional summary entry with tags
/// `conversation, summary` is created and linked to every exchange via
/// an [`RelationKind::Elaborates`] relation (summary → exchange).
///
/// Partial-failure semantics match the MCP handler this was lifted from:
/// the first storage error propagates and any exchanges already written
/// remain in the store. Callers that need strict all-or-nothing
/// semantics must wrap the call in their own transaction.
pub async fn deposit(
    store: &impl ContextStore,
    request: DepositRequest,
    ctx: &WriteContext,
) -> Result<DepositResult, CmError> {
    if request.exchanges.is_empty() {
        return Err(CmError::Validation("exchanges cannot be empty".to_owned()));
    }
    if request.exchanges.len() > MAX_EXCHANGES {
        return Err(CmError::Validation(format!(
            "maximum {MAX_EXCHANGES} exchanges per deposit"
        )));
    }

    // Validate individual exchange sizes and explicit titles before any
    // write so callers see early-failure input errors rather than a
    // half-written batch.
    for (i, ex) in request.exchanges.iter().enumerate() {
        check_input_size(&ex.user, &format!("exchanges[{i}].user")).map_err(CmError::Validation)?;
        check_input_size(&ex.assistant, &format!("exchanges[{i}].assistant"))
            .map_err(CmError::Validation)?;
        if let Some(ref t) = ex.title
            && (t.is_empty() || t.len() > EXCHANGE_TITLE_LEN)
        {
            return Err(CmError::Validation(format!(
                "exchanges[{i}].title must be 1-{EXCHANGE_TITLE_LEN} bytes"
            )));
        }
    }

    let scope_selector = request
        .scope
        .unwrap_or_else(|| ScopeSelector::Path(ScopePath::global()));
    let resolved_scope = resolve_scope_selection(store, &scope_selector).await?;
    let scope_path = resolved_scope.write_scope_path()?.clone();

    ensure_scope_chain_with_status(store, &scope_path, ctx).await?;

    let mut entry_ids = Vec::with_capacity(request.exchanges.len());

    // One entry per exchange.
    for ex in &request.exchanges {
        let title = match &ex.title {
            Some(t) => t.clone(),
            None => snippet(&ex.user, EXCHANGE_TITLE_LEN),
        };
        let body = format!("{}\n\n---\n\n{}", ex.user, ex.assistant);

        let new_entry = NewEntry {
            scope_path: scope_path.clone(),
            kind: EntryKind::Observation,
            title,
            body,
            created_by: request.created_by.clone(),
            meta: Some(EntryMeta {
                tags: vec!["conversation".to_owned()],
                ..EntryMeta::default()
            }),
        };

        let entry = store.create_entry(new_entry, ctx).await?;
        entry_ids.push(entry.id);
    }

    // Optional summary entry. Created after all exchanges so the
    // `elaborates` edges can point at real IDs; the cost of an empty
    // `summary` is zero extra writes (the `None` branch short-circuits).
    let summary_id = if let Some(ref summary_text) = request.summary {
        check_input_size(summary_text, "summary").map_err(CmError::Validation)?;

        let summary_entry = NewEntry {
            scope_path: scope_path.clone(),
            kind: EntryKind::Observation,
            title: "Session summary".to_owned(),
            body: summary_text.clone(),
            created_by: request.created_by.clone(),
            meta: Some(EntryMeta {
                tags: vec!["conversation".to_owned(), "summary".to_owned()],
                ..EntryMeta::default()
            }),
        };

        let entry = store.create_entry(summary_entry, ctx).await?;
        let sid = entry.id;

        // Link summary → each exchange via `elaborates`. Matches the
        // semantics of the MCP handler this was lifted from.
        for &exchange_id in &entry_ids {
            store
                .create_relation(sid, exchange_id, RelationKind::Elaborates, ctx)
                .await?;
        }

        Some(sid)
    } else {
        None
    };

    Ok(DepositResult {
        entry_ids,
        summary_id,
        scope_path: scope_path.as_str().to_owned(),
    })
}
