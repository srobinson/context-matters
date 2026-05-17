use cm_core::{CmError, ContextStore, NewScope, ScopeKind, ScopePath, WriteContext};

use crate::error::cm_err_to_string;

use super::segments::scope_segments;

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

    reject_colliding_scope_auto_creation(store, path, &missing).await?;

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

async fn reject_colliding_scope_auto_creation(
    store: &impl ContextStore,
    requested_path: &ScopePath,
    missing: &[ScopePath],
) -> Result<(), CmError> {
    let checks_project_or_repo = missing
        .iter()
        .any(|path| matches!(path.leaf_kind(), ScopeKind::Project | ScopeKind::Repo));
    if !checks_project_or_repo {
        return Ok(());
    }

    let requested = scope_segments(requested_path);
    let Some(requested_project) = requested.project.as_deref() else {
        return Ok(());
    };
    let Some(colliding_scope) =
        colliding_existing_repo_scope(store, requested_project, requested.repo.as_deref()).await?
    else {
        return Ok(());
    };

    Err(CmError::Validation(format!(
        "refusing to auto-create scope '{}' because it collides with existing repo scope '{}'; use the existing scope or choose a non-colliding project/repo name",
        requested_path.as_str(),
        colliding_scope.as_str()
    )))
}

async fn colliding_existing_repo_scope(
    store: &impl ContextStore,
    requested_project: &str,
    requested_repo: Option<&str>,
) -> Result<Option<ScopePath>, CmError> {
    let scopes = store.list_scopes(Some(ScopeKind::Repo)).await?;
    for scope in scopes {
        let segments = scope_segments(&scope.path);
        let Some(existing_repo) = segments.repo.as_deref() else {
            continue;
        };
        if segments.project.as_deref() == Some(requested_project) {
            continue;
        }
        if Some(existing_repo) == requested_repo || existing_repo == requested_project {
            return Ok(Some(scope.path));
        }
    }
    Ok(None)
}

fn scope_label(path: &ScopePath) -> String {
    path.as_str()
        .rsplit('/')
        .next()
        .and_then(|s| s.split(':').nth(1))
        .unwrap_or(path.as_str())
        .to_owned()
}
