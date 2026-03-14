//! Tool handlers for the 9 `cx_*` tools.
//!
//! Each handler receives a reference to the store and the raw JSON arguments,
//! validates inputs, calls the appropriate `ContextStore` trait methods, and
//! returns a pretty-printed JSON string or an error message with recovery guidance.

use cm_core::{
    Confidence, ContextStore, Entry, EntryFilter, EntryKind, EntryMeta, NewEntry, Pagination,
    RelationKind, ScopePath, UpdateEntry,
};
use cm_store::CmStore;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    SNIPPET_LENGTH, check_input_size, clamp_limit, cm_err_to_string, decode_cursor, encode_cursor,
    ensure_scope_chain, estimate_tokens, json_response, snippet,
};

// ── cx_recall ────────────────────────────────────────────────────

/// Parameters for the `cx_recall` tool.
#[derive(Debug, Deserialize)]
struct CxRecallParams {
    /// FTS5 search query. When omitted, uses scope resolution instead.
    #[serde(default)]
    query: Option<String>,

    /// Scope path to search within. Defaults to "global".
    #[serde(default)]
    scope: Option<String>,

    /// Filter to specific entry kinds (OR semantics).
    #[serde(default)]
    kinds: Vec<String>,

    /// Filter to entries with any of these tags (OR semantics).
    #[serde(default)]
    tags: Vec<String>,

    /// Maximum number of entries to return.
    #[serde(default)]
    limit: Option<u32>,

    /// Maximum token budget for the response.
    #[serde(default)]
    max_tokens: Option<u32>,
}

pub fn cx_recall(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxRecallParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    // Validate query size if provided
    if let Some(ref q) = params.query {
        check_input_size(q, "query")?;
    }

    // Parse and validate scope path
    let scope_path = match &params.scope {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };
    let default_scope = ScopePath::global();
    let scope_ref = scope_path.as_ref().unwrap_or(&default_scope);

    // Parse kind filters
    let kind_filters: Vec<EntryKind> = params
        .kinds
        .iter()
        .map(|k| k.parse::<EntryKind>().map_err(cm_err_to_string))
        .collect::<Result<Vec<_>, _>>()?;

    let limit = clamp_limit(params.limit);

    // Fetch more than requested when post-filtering, to compensate for filtered-out entries
    let has_post_filter = !kind_filters.is_empty() || !params.tags.is_empty();
    let fetch_limit = if has_post_filter {
        limit.saturating_mul(3).min(super::MAX_LIMIT)
    } else {
        limit
    };

    // Route to search or resolve_context based on query presence
    let entries = match &params.query {
        Some(query) => store
            .search(query, Some(scope_ref), fetch_limit)
            .map_err(cm_err_to_string)?,
        None => store
            .resolve_context(scope_ref, &kind_filters, fetch_limit)
            .map_err(cm_err_to_string)?,
    };

    // Post-filter by kinds (only when using search path, since resolve_context handles kinds internally)
    let entries = if params.query.is_some() && !kind_filters.is_empty() {
        entries
            .into_iter()
            .filter(|e| kind_filters.contains(&e.kind))
            .collect()
    } else {
        entries
    };

    // Post-filter by tags
    let entries: Vec<Entry> = if params.tags.is_empty() {
        entries
    } else {
        entries
            .into_iter()
            .filter(|e| entry_has_any_tag(e, &params.tags))
            .collect()
    };

    // Apply limit after post-filtering
    let entries: Vec<Entry> = entries.into_iter().take(limit as usize).collect();

    // Build scope chain from the target scope
    let scope_chain: Vec<String> = scope_ref.ancestors().map(String::from).collect();

    // Build result entries with token budget tracking
    let mut results = Vec::with_capacity(entries.len());
    let mut total_tokens: u32 = 0;

    for entry in &entries {
        let entry_json = entry_to_recall_json(entry);
        let entry_tokens = estimate_tokens(&entry_json.to_string());

        if let Some(budget) = params.max_tokens
            && total_tokens + entry_tokens > budget
            && !results.is_empty()
        {
            break;
        }

        total_tokens += entry_tokens;
        results.push(entry_json);
    }

    let response = json!({
        "results": results,
        "returned": results.len(),
        "scope_chain": scope_chain,
        "token_estimate": total_tokens,
    });

    json_response(response)
}

/// Convert an entry to the two-phase recall response format (snippet, not full body).
fn entry_to_recall_json(entry: &Entry) -> Value {
    let mut result = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "snippet": snippet(&entry.body, SNIPPET_LENGTH),
        "created_by": &entry.created_by,
        "updated_at": entry.updated_at.to_rfc3339(),
    });

    if let Some(ref meta) = entry.meta {
        if !meta.tags.is_empty() {
            result["tags"] = json!(meta.tags);
        }
        if let Some(ref confidence) = meta.confidence {
            result["confidence"] = json!(confidence);
        }
    }

    result
}

/// Check whether an entry has any of the specified tags.
fn entry_has_any_tag(entry: &Entry, tags: &[String]) -> bool {
    match &entry.meta {
        Some(meta) => meta.tags.iter().any(|t| tags.contains(t)),
        None => false,
    }
}

// ── Stubs for remaining tools ────────────────────────────────────

// ── cx_store ─────────────────────────────────────────────────────

/// Parameters for the `cx_store` tool.
#[derive(Debug, Deserialize)]
struct CxStoreParams {
    /// Short summary displayed in search results.
    title: String,

    /// Full content body in markdown.
    body: String,

    /// Entry classification.
    kind: String,

    /// Target scope path. Auto-created if missing.
    #[serde(default = "default_scope")]
    scope_path: String,

    /// Attribution string.
    #[serde(default = "default_created_by")]
    created_by: String,

    /// Freeform tags.
    #[serde(default)]
    tags: Vec<String>,

    /// Confidence level: "high", "medium", or "low".
    #[serde(default)]
    confidence: Option<String>,

    /// Source URL or path.
    #[serde(default)]
    source: Option<String>,

    /// ISO 8601 expiry timestamp.
    #[serde(default)]
    expires_at: Option<String>,

    /// Numeric priority for manual ordering.
    #[serde(default)]
    priority: Option<i32>,

    /// ID of an existing entry that this new entry supersedes.
    #[serde(default)]
    supersedes: Option<String>,
}

fn default_scope() -> String {
    "global".to_owned()
}

fn default_created_by() -> String {
    "agent:claude-code".to_owned()
}

pub fn cx_store(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxStoreParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    // Validate input sizes
    check_input_size(&params.title, "title")?;
    check_input_size(&params.body, "body")?;

    // Parse scope path and entry kind
    let scope_path =
        ScopePath::parse(&params.scope_path).map_err(|e| cm_err_to_string(e.into()))?;
    let kind: EntryKind = params.kind.parse().map_err(cm_err_to_string)?;

    // Parse confidence if provided
    let confidence = match &params.confidence {
        Some(c) => Some(parse_confidence(c)?),
        None => None,
    };

    // Parse expires_at if provided
    let expires_at = match &params.expires_at {
        Some(s) => Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| format!("Invalid expires_at: {e}. Expected ISO 8601 format."))?,
        ),
        None => None,
    };

    // Auto-create scope chain if needed
    ensure_scope_chain(store, &scope_path)?;

    // Build metadata
    let meta = if !params.tags.is_empty()
        || confidence.is_some()
        || params.source.is_some()
        || expires_at.is_some()
        || params.priority.is_some()
    {
        Some(EntryMeta {
            tags: params.tags,
            confidence,
            source: params.source,
            expires_at,
            priority: params.priority,
            extra: std::collections::HashMap::new(),
        })
    } else {
        None
    };

    let new_entry = NewEntry {
        scope_path,
        kind,
        title: params.title,
        body: params.body,
        created_by: params.created_by,
        meta,
    };

    // Create or supersede
    let (entry, superseded_id) = match params.supersedes {
        Some(ref id_str) => {
            let old_id = uuid::Uuid::parse_str(id_str)
                .map_err(|_| format!("Invalid supersedes ID: '{id_str}'. Expected a UUID."))?;
            let entry = store
                .supersede_entry(old_id, new_entry)
                .map_err(cm_err_to_string)?;
            (entry, Some(old_id))
        }
        None => {
            let entry = store.create_entry(new_entry).map_err(cm_err_to_string)?;
            (entry, None)
        }
    };

    let message = match superseded_id {
        Some(old_id) => format!("Entry stored. Superseded entry {old_id}."),
        None => "Entry stored.".to_owned(),
    };

    let response = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "content_hash": &entry.content_hash,
        "created_at": entry.created_at.to_rfc3339(),
        "superseded": superseded_id.map(|id| id.to_string()),
        "message": message,
    });

    json_response(response)
}

/// Parse a confidence string to the Confidence enum.
fn parse_confidence(s: &str) -> Result<Confidence, String> {
    match s {
        "high" => Ok(Confidence::High),
        "medium" => Ok(Confidence::Medium),
        "low" => Ok(Confidence::Low),
        other => Err(format!(
            "Invalid confidence '{other}'. Valid values: high, medium, low."
        )),
    }
}

// ── cx_deposit ───────────────────────────────────────────────────

/// Maximum exchanges per deposit call.
const MAX_EXCHANGES: usize = 50;

/// Title truncation length for exchange entries.
const EXCHANGE_TITLE_LEN: usize = 80;

#[derive(Debug, Deserialize)]
struct CxDepositParams {
    /// Conversation exchanges to store.
    exchanges: Vec<Exchange>,

    /// Optional summary linked to all exchange entries.
    #[serde(default)]
    summary: Option<String>,

    /// Target scope path. Default: "global".
    #[serde(default = "default_scope")]
    scope_path: String,

    /// Attribution. Default: "agent:claude-code".
    #[serde(default = "default_created_by")]
    created_by: String,
}

#[derive(Debug, Deserialize)]
struct Exchange {
    user: String,
    assistant: String,
}

pub fn cx_deposit(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxDepositParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    if params.exchanges.is_empty() {
        return Err("Validation error: exchanges cannot be empty".to_owned());
    }
    if params.exchanges.len() > MAX_EXCHANGES {
        return Err(format!(
            "Validation error: maximum {MAX_EXCHANGES} exchanges per deposit"
        ));
    }

    // Validate individual exchange sizes
    for (i, ex) in params.exchanges.iter().enumerate() {
        check_input_size(&ex.user, &format!("exchanges[{i}].user"))?;
        check_input_size(&ex.assistant, &format!("exchanges[{i}].assistant"))?;
    }

    let scope_path =
        ScopePath::parse(&params.scope_path).map_err(|e| cm_err_to_string(e.into()))?;

    // Auto-create scope chain
    ensure_scope_chain(store, &scope_path)?;

    let mut entry_ids = Vec::with_capacity(params.exchanges.len());

    // Create one entry per exchange
    for ex in &params.exchanges {
        let title = snippet(&ex.user, EXCHANGE_TITLE_LEN);
        let body = format!("{}\n\n---\n\n{}", ex.user, ex.assistant);

        let new_entry = NewEntry {
            scope_path: scope_path.clone(),
            kind: EntryKind::Observation,
            title,
            body,
            created_by: params.created_by.clone(),
            meta: Some(EntryMeta {
                tags: vec!["conversation".to_owned()],
                ..EntryMeta::default()
            }),
        };

        let entry = store.create_entry(new_entry).map_err(cm_err_to_string)?;
        entry_ids.push(entry.id);
    }

    // Create summary entry and link to exchanges
    let summary_id = if let Some(ref summary_text) = params.summary {
        check_input_size(summary_text, "summary")?;

        let summary_entry = NewEntry {
            scope_path: scope_path.clone(),
            kind: EntryKind::Observation,
            title: "Session summary".to_owned(),
            body: summary_text.clone(),
            created_by: params.created_by.clone(),
            meta: Some(EntryMeta {
                tags: vec!["conversation".to_owned(), "summary".to_owned()],
                ..EntryMeta::default()
            }),
        };

        let entry = store
            .create_entry(summary_entry)
            .map_err(cm_err_to_string)?;
        let sid = entry.id;

        // Link summary to each exchange via elaborates relation
        for &exchange_id in &entry_ids {
            store
                .create_relation(sid, exchange_id, RelationKind::Elaborates)
                .map_err(cm_err_to_string)?;
        }

        Some(sid)
    } else {
        None
    };

    let count = entry_ids.len();
    let message = match summary_id {
        Some(_) => format!("Deposited {count} exchanges with summary."),
        None => format!("Deposited {count} exchanges."),
    };

    let response = json!({
        "deposited": count,
        "entry_ids": entry_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "summary_id": summary_id.map(|id| id.to_string()),
        "message": message,
    });

    json_response(response)
}

// ── cx_browse ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CxBrowseParams {
    /// Filter to entries at this exact scope path (no ancestor walk).
    #[serde(default)]
    scope_path: Option<String>,

    /// Filter by entry kind.
    #[serde(default)]
    kind: Option<String>,

    /// Filter by tag.
    #[serde(default)]
    tag: Option<String>,

    /// Filter by creator attribution.
    #[serde(default)]
    created_by: Option<String>,

    /// Include superseded entries.
    #[serde(default)]
    include_superseded: bool,

    /// Maximum entries per page.
    #[serde(default)]
    limit: Option<u32>,

    /// Opaque pagination cursor from a previous response.
    #[serde(default)]
    cursor: Option<String>,
}

pub fn cx_browse(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxBrowseParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    let scope_path = match &params.scope_path {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    let kind = match &params.kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(cm_err_to_string)?),
        None => None,
    };

    let cursor = match &params.cursor {
        Some(c) => Some(decode_cursor(c)?),
        None => None,
    };

    let limit = clamp_limit(params.limit);

    let filter = EntryFilter {
        scope_path,
        kind,
        tag: params.tag,
        created_by: params.created_by,
        include_superseded: params.include_superseded,
        pagination: Pagination { limit, cursor },
    };

    let result = store.browse(filter).map_err(cm_err_to_string)?;

    let entries: Vec<Value> = result.items.iter().map(entry_to_browse_json).collect();

    let next_cursor = result.next_cursor.as_ref().map(encode_cursor);
    let has_more = next_cursor.is_some();

    let response = json!({
        "entries": entries,
        "total": result.total,
        "next_cursor": next_cursor,
        "has_more": has_more,
    });

    json_response(response)
}

/// Convert an entry to the browse response format (two-phase: snippet, not full body).
fn entry_to_browse_json(entry: &Entry) -> Value {
    let mut result = json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "snippet": snippet(&entry.body, SNIPPET_LENGTH),
        "created_by": &entry.created_by,
        "created_at": entry.created_at.to_rfc3339(),
        "updated_at": entry.updated_at.to_rfc3339(),
        "superseded_by": entry.superseded_by.map(|id| id.to_string()),
    });

    if let Some(ref meta) = entry.meta
        && !meta.tags.is_empty()
    {
        result["tags"] = json!(meta.tags);
    }

    result
}

// ── cx_get ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CxGetParams {
    /// Entry IDs to retrieve. Maximum 100 per request.
    ids: Vec<String>,
}

pub fn cx_get(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxGetParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    if params.ids.is_empty() {
        return Err("Validation error: ids cannot be empty".to_owned());
    }
    if params.ids.len() > super::MAX_BATCH_IDS {
        return Err(format!(
            "Validation error: maximum {} IDs per request",
            super::MAX_BATCH_IDS
        ));
    }

    let uuids: Vec<uuid::Uuid> = params
        .ids
        .iter()
        .map(|s| uuid::Uuid::parse_str(s).map_err(|_| format!("Invalid UUID format: '{s}'")))
        .collect::<Result<Vec<_>, _>>()?;

    let entries = store.get_entries(&uuids).map_err(cm_err_to_string)?;
    let found = entries.len();
    let missing = uuids.len() - found;

    let entries_json: Vec<Value> = entries.iter().map(entry_to_full_json).collect();

    let response = json!({
        "entries": entries_json,
        "found": found,
        "missing": missing,
    });

    json_response(response)
}

/// Convert an entry to the full response format (includes body).
fn entry_to_full_json(entry: &Entry) -> Value {
    json!({
        "id": entry.id.to_string(),
        "scope_path": entry.scope_path.as_str(),
        "kind": entry.kind.as_str(),
        "title": &entry.title,
        "body": &entry.body,
        "content_hash": &entry.content_hash,
        "meta": &entry.meta,
        "created_by": &entry.created_by,
        "created_at": entry.created_at.to_rfc3339(),
        "updated_at": entry.updated_at.to_rfc3339(),
        "superseded_by": entry.superseded_by.map(|id| id.to_string()),
    })
}

// ── cx_update ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CxUpdateParams {
    /// ID of the entry to update.
    id: String,

    /// New title.
    #[serde(default)]
    title: Option<String>,

    /// New body content.
    #[serde(default)]
    body: Option<String>,

    /// New kind classification.
    #[serde(default)]
    kind: Option<String>,

    /// Replace metadata entirely.
    #[serde(default)]
    meta: Option<CxMetaInput>,
}

#[derive(Debug, Deserialize)]
struct CxMetaInput {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    priority: Option<i32>,
}

pub fn cx_update(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxUpdateParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    let id = uuid::Uuid::parse_str(&params.id)
        .map_err(|_| format!("Invalid UUID format: '{}'", params.id))?;

    // Validate at least one field is provided
    if params.title.is_none()
        && params.body.is_none()
        && params.kind.is_none()
        && params.meta.is_none()
    {
        return Err("Validation error: at least one field must be provided".to_owned());
    }

    // Validate input sizes
    if let Some(ref t) = params.title {
        check_input_size(t, "title")?;
    }
    if let Some(ref b) = params.body {
        check_input_size(b, "body")?;
    }

    // Parse kind if provided
    let kind = match &params.kind {
        Some(k) => Some(k.parse::<EntryKind>().map_err(cm_err_to_string)?),
        None => None,
    };

    // Parse meta if provided
    let meta = match params.meta {
        Some(m) => {
            let confidence = match &m.confidence {
                Some(c) => Some(parse_confidence(c)?),
                None => None,
            };
            let expires_at = match &m.expires_at {
                Some(s) => Some(
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .map_err(|e| {
                            format!("Invalid expires_at: {e}. Expected ISO 8601 format.")
                        })?,
                ),
                None => None,
            };
            Some(EntryMeta {
                tags: m.tags,
                confidence,
                source: m.source,
                expires_at,
                priority: m.priority,
                extra: std::collections::HashMap::new(),
            })
        }
        None => None,
    };

    let update = UpdateEntry {
        title: params.title,
        body: params.body,
        kind,
        meta,
    };

    let entry = store.update_entry(id, update).map_err(cm_err_to_string)?;

    let response = json!({
        "entry": {
            "id": entry.id.to_string(),
            "scope_path": entry.scope_path.as_str(),
            "kind": entry.kind.as_str(),
            "title": &entry.title,
            "content_hash": &entry.content_hash,
            "updated_at": entry.updated_at.to_rfc3339(),
        },
        "message": "Entry updated.",
    });

    json_response(response)
}

// ── cx_forget ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CxForgetParams {
    /// Entry IDs to forget. Maximum 100 per request.
    ids: Vec<String>,
}

pub fn cx_forget(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxForgetParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    if params.ids.is_empty() {
        return Err("Validation error: ids cannot be empty".to_owned());
    }
    if params.ids.len() > super::MAX_BATCH_IDS {
        return Err(format!(
            "Validation error: maximum {} IDs per request",
            super::MAX_BATCH_IDS
        ));
    }

    let uuids: Vec<uuid::Uuid> = params
        .ids
        .iter()
        .map(|s| uuid::Uuid::parse_str(s).map_err(|_| format!("Invalid UUID format: '{s}'")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut forgotten = 0u32;
    let mut already_inactive = 0u32;
    let mut not_found = 0u32;
    let mut details = Vec::with_capacity(uuids.len());

    for &id in &uuids {
        // Check current state
        match store.get_entry(id) {
            Ok(entry) => {
                if entry.superseded_by.is_some() {
                    already_inactive += 1;
                    details.push(json!({"id": id.to_string(), "status": "already_inactive"}));
                } else {
                    match store.forget_entry(id) {
                        Ok(()) => {
                            forgotten += 1;
                            details.push(json!({"id": id.to_string(), "status": "forgotten"}));
                        }
                        Err(e) => {
                            details.push(json!({"id": id.to_string(), "status": "error", "error": cm_err_to_string(e)}));
                        }
                    }
                }
            }
            Err(cm_core::CmError::EntryNotFound(_)) => {
                not_found += 1;
                details.push(json!({"id": id.to_string(), "status": "not_found"}));
            }
            Err(e) => {
                details.push(
                    json!({"id": id.to_string(), "status": "error", "error": cm_err_to_string(e)}),
                );
            }
        }
    }

    let mut parts = Vec::new();
    if forgotten > 0 {
        parts.push(format!("Forgot {forgotten} entries."));
    }
    if already_inactive > 0 {
        parts.push(format!("{already_inactive} already inactive."));
    }
    if not_found > 0 {
        parts.push(format!("{not_found} not found."));
    }
    let message = if parts.is_empty() {
        "No entries processed.".to_owned()
    } else {
        parts.join(" ")
    };

    let response = json!({
        "forgotten": forgotten,
        "already_inactive": already_inactive,
        "not_found": not_found,
        "details": details,
        "message": message,
    });

    json_response(response)
}

// ── cx_stats ─────────────────────────────────────────────────────

pub fn cx_stats(store: &CmStore, _args: &Value) -> Result<String, String> {
    let stats = store.stats().map_err(cm_err_to_string)?;
    let scopes = store.list_scopes(None).map_err(cm_err_to_string)?;

    let scope_tree: Vec<Value> = scopes
        .iter()
        .map(|s| {
            let entry_count = stats
                .entries_by_scope
                .get(s.path.as_str())
                .copied()
                .unwrap_or(0);
            json!({
                "path": s.path.as_str(),
                "kind": s.kind.as_str(),
                "label": &s.label,
                "entry_count": entry_count,
            })
        })
        .collect();

    let response = json!({
        "active_entries": stats.active_entries,
        "superseded_entries": stats.superseded_entries,
        "scopes": stats.scopes,
        "relations": stats.relations,
        "entries_by_kind": stats.entries_by_kind,
        "entries_by_scope": stats.entries_by_scope,
        "db_size_bytes": stats.db_size_bytes,
        "scope_tree": scope_tree,
    });

    json_response(response)
}

// ── cx_export ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CxExportParams {
    /// Filter to a specific scope path.
    #[serde(default)]
    scope_path: Option<String>,

    /// Export format. Only "json" supported.
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "json".to_owned()
}

pub fn cx_export(store: &CmStore, args: &Value) -> Result<String, String> {
    let params: CxExportParams =
        serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))?;

    if params.format != "json" {
        return Err(format!(
            "Unsupported export format '{}'. Currently only 'json' is supported.",
            params.format
        ));
    }

    let scope_path = match &params.scope_path {
        Some(s) => Some(ScopePath::parse(s).map_err(|e| cm_err_to_string(e.into()))?),
        None => None,
    };

    let entries = store
        .export(scope_path.as_ref())
        .map_err(cm_err_to_string)?;

    let all_scopes = store.list_scopes(None).map_err(cm_err_to_string)?;

    // Filter scopes by prefix if scope_path is specified
    let scopes: Vec<_> = match &scope_path {
        Some(sp) => all_scopes
            .into_iter()
            .filter(|s| s.path.as_str().starts_with(sp.as_str()))
            .collect(),
        None => all_scopes,
    };

    let count = entries.len();

    let response = json!({
        "entries": entries,
        "scopes": scopes,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "count": count,
    });

    json_response(response)
}
