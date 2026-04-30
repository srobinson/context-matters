//! Content-first search capability.

use cm_core::{CmError, ContentSearchPage, ContentSearchRequest, ContextStore, FtsQuery};

/// Execute content-first search against the store.
///
/// `cx_search` requires a non-empty FTS query. Use `cx_browse` for
/// listing or filtering entries without free text.
pub async fn search(
    store: &impl ContextStore,
    request: ContentSearchRequest,
) -> Result<ContentSearchPage, CmError> {
    if FtsQuery::new(&request.query).as_str().is_empty() {
        return Err(CmError::InvalidOperationInput {
            op: "cx_search",
            reason: "query is required; use cx_browse to list or filter without a query".to_owned(),
        });
    }

    store.do_content_search(request).await
}
