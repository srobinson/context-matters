use cm_core::{CmError, ContextStore, NewScope, ScopePath, WriteContext};

/// Ensure the full scope chain exists and report whether any scope was created.
///
/// Uniqueness is enforced by the scopes table primary key on the full
/// `ScopePath`. Two scopes that share a leaf identifier under different
/// ancestor chains are distinct scopes and may coexist; nesting a deeper
/// project under a shared ancestor is therefore permitted even when a
/// sibling scope shares the same leaf name.
pub async fn ensure_scope_chain_with_status(
    store: &impl ContextStore,
    path: &ScopePath,
    ctx: &WriteContext,
) -> Result<bool, CmError> {
    let ancestors: Vec<ScopePath> = path
        .ancestors()
        .map(ScopePath::parse)
        .collect::<Result<_, _>>()?;
    let mut missing = Vec::new();

    for ancestor_str in ancestors.into_iter().rev() {
        match store.get_scope(&ancestor_str).await {
            Ok(_) => continue,
            Err(CmError::ScopeNotFound(_)) => {
                missing.push(ancestor_str);
            }
            Err(e) => return Err(e),
        }
    }

    for ancestor in &missing {
        let new_scope = NewScope {
            path: ancestor.clone(),
            label: scope_label(ancestor),
            meta: None,
        };
        store.create_scope(new_scope, ctx).await?;
    }

    Ok(!missing.is_empty())
}

fn scope_label(path: &ScopePath) -> String {
    path.as_str()
        .rsplit('/')
        .next()
        .and_then(|s| s.split(':').nth(1))
        .unwrap_or(path.as_str())
        .to_owned()
}
