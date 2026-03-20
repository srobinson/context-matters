//! Handler for the `cx_deposit` tool.

use cm_core::{
    ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, RelationKind, ScopePath,
    WriteContext,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::{
    check_input_size, cm_err_to_string, ensure_scope_chain, json_response, parse_params, snippet,
};

use super::{default_created_by, default_scope};

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
    #[serde(default)]
    title: Option<String>,
}

pub async fn cx_deposit(store: &impl ContextStore, args: &Value) -> Result<String, String> {
    let ctx = WriteContext::new(MutationSource::Mcp);

    let params: CxDepositParams = parse_params(args)?;

    if params.exchanges.is_empty() {
        return Err("Validation error: exchanges cannot be empty".to_owned());
    }
    if params.exchanges.len() > MAX_EXCHANGES {
        return Err(format!(
            "Validation error: maximum {MAX_EXCHANGES} exchanges per deposit"
        ));
    }

    // Validate individual exchange sizes and explicit titles
    for (i, ex) in params.exchanges.iter().enumerate() {
        check_input_size(&ex.user, &format!("exchanges[{i}].user"))?;
        check_input_size(&ex.assistant, &format!("exchanges[{i}].assistant"))?;
        if let Some(ref t) = ex.title
            && (t.is_empty() || t.len() > EXCHANGE_TITLE_LEN)
        {
            return Err(format!(
                "Validation error: exchanges[{i}].title must be 1-{EXCHANGE_TITLE_LEN} bytes"
            ));
        }
    }

    let scope_path =
        ScopePath::parse(&params.scope_path).map_err(|e| cm_err_to_string(e.into()))?;

    // Auto-create scope chain
    ensure_scope_chain(store, &scope_path, &ctx).await?;

    let mut entry_ids = Vec::with_capacity(params.exchanges.len());

    // Create one entry per exchange
    for ex in &params.exchanges {
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
            created_by: params.created_by.clone(),
            meta: Some(EntryMeta {
                tags: vec!["conversation".to_owned()],
                ..EntryMeta::default()
            }),
        };

        let entry = store
            .create_entry(new_entry, &ctx)
            .await
            .map_err(cm_err_to_string)?;
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
            .create_entry(summary_entry, &ctx)
            .await
            .map_err(cm_err_to_string)?;
        let sid = entry.id;

        // Link summary to each exchange via elaborates relation
        for &exchange_id in &entry_ids {
            store
                .create_relation(sid, exchange_id, RelationKind::Elaborates, &ctx)
                .await
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
