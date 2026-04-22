use cm_core::{CmError, ContextStore, NewScope, ScopePath, WriteContext};

use crate::error::cm_err_to_string;

/// Ensure the full scope chain exists, creating missing scopes top-down.
///
/// When creating an entry with a scope path that does not exist, this
/// function creates the full scope chain automatically. This prevents
/// callers from needing to manage scope creation separately.
pub async fn ensure_scope_chain(
    store: &impl ContextStore,
    path: &ScopePath,
    ctx: &WriteContext,
) -> Result<(), String> {
    ensure_scope_chain_with_status(store, path, ctx)
        .await
        .map(|_| ())
        .map_err(cm_err_to_string)
}

/// Ensure the full scope chain exists and report whether any scope was created.
pub async fn ensure_scope_chain_with_status(
    store: &impl ContextStore,
    path: &ScopePath,
    ctx: &WriteContext,
) -> Result<bool, CmError> {
    let ancestors: Vec<&str> = path.ancestors().collect();
    let mut created = false;

    for ancestor_str in ancestors.into_iter().rev() {
        let ancestor = ScopePath::parse(ancestor_str)?;
        match store.get_scope(&ancestor).await {
            Ok(_) => continue,
            Err(CmError::ScopeNotFound(_)) => {
                let label = ancestor_str
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.split(':').nth(1))
                    .unwrap_or(ancestor_str)
                    .to_owned();

                let new_scope = NewScope {
                    path: ancestor,
                    label,
                    meta: None,
                };
                store.create_scope(new_scope, ctx).await?;
                created = true;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(created)
}
