//! Content-first search capability.

use cm_core::{CmError, ContentSearchPage, ContentSearchRequest, ContextStore, FtsQuery};

use crate::telemetry::RetrievalLog;

/// Execute content-first search against the store.
///
/// `cx_search` requires a non-empty FTS query. Use `cx_browse` for
/// listing or filtering entries without free text.
pub async fn search(
    store: &impl ContextStore,
    request: ContentSearchRequest,
) -> Result<ContentSearchPage, CmError> {
    let log = RetrievalLog::from_search_request(&request);
    let result = search_inner(store, request).await;
    log.emit_search(&result);
    result
}

async fn search_inner(
    store: &impl ContextStore,
    request: ContentSearchRequest,
) -> Result<ContentSearchPage, CmError> {
    if FtsQuery::new(&request.query).as_str().is_empty() {
        return Err(if request.query.trim().is_empty() {
            missing_search_query()
        } else {
            invalid_search_query()
        });
    }

    store
        .do_content_search(request)
        .await
        .map_err(map_content_search_error)
}

fn map_content_search_error(err: CmError) -> CmError {
    match err {
        CmError::Database(message) if is_fts_syntax_error(&message) => invalid_search_query(),
        other => other,
    }
}

fn is_fts_syntax_error(message: &str) -> bool {
    message.contains("fts5: syntax error")
}

fn invalid_search_query() -> CmError {
    CmError::InvalidOperationInput {
        op: "cx_search",
        reason: "query is invalid; use cx_browse to list or filter without a query".to_owned(),
    }
}

fn missing_search_query() -> CmError {
    CmError::InvalidOperationInput {
        op: "cx_search",
        reason: "query is required; use cx_browse to list or filter without a query".to_owned(),
    }
}
