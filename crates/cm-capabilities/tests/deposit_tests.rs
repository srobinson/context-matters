mod common;

use cm_capabilities::deposit::{DepositRequest, Exchange, deposit};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{CmError, ContextStore, MutationSource, ScopePath, WriteContext};

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

fn exchange() -> Exchange {
    Exchange {
        user: "What changed?".to_owned(),
        assistant: "Scope selection changed.".to_owned(),
        title: None,
    }
}

fn request(scope: Option<ScopeSelector>) -> DepositRequest {
    DepositRequest {
        exchanges: vec![exchange()],
        summary: None,
        scope,
        created_by: "agent:test".to_owned(),
    }
}

fn assert_validation(err: CmError, expected: &str) {
    match err {
        CmError::Validation(msg) => assert_eq!(msg, expected),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_defaults_to_global_scope() {
    let (store, _dir) = common::test_store().await;
    common::create_global(&store).await;

    let result = deposit(&store, request(None), &wctx()).await.unwrap();

    assert_eq!(result.scope_path, "global");
    let entries = store.export(None).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].scope_path.as_str(), "global");
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_stores_exact_scope_selector() {
    let (store, _dir) = common::test_store().await;
    common::create_global(&store).await;
    let scope = ScopePath::parse("global/project:helioy/repo:context-matters").unwrap();

    let result = deposit(
        &store,
        request(Some(ScopeSelector::Path(scope.clone()))),
        &wctx(),
    )
    .await
    .unwrap();

    assert_eq!(result.scope_path, scope.as_str());
    let entries = store.export(None).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].scope_path, scope);
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_resolves_cwd_inferred_scope_before_writing() {
    let (store, _dir) = common::test_store().await;
    common::ensure_scope(&store, "global/project:helioy/repo:context-matters").await;

    let result = deposit(
        &store,
        request(Some(ScopeSelector::cwd_inferred(Some(
            "/tmp/helioy/context-matters".into(),
        )))),
        &wctx(),
    )
    .await
    .unwrap();

    assert_eq!(
        result.scope_path,
        "global/project:helioy/repo:context-matters"
    );
    let entries = store.export(None).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].scope_path.as_str(),
        "global/project:helioy/repo:context-matters"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_rejects_medium_confidence_cwd_inferred_without_partial_write() {
    let (store, _dir) = common::test_store().await;
    common::ensure_scope(&store, "global/project:helioy").await;
    let scope_count = store.list_scopes(None).await.unwrap().len();

    let err = deposit(
        &store,
        request(Some(ScopeSelector::cwd_inferred(Some(
            "/tmp/helioy/context-matters".into(),
        )))),
        &wctx(),
    )
    .await
    .unwrap_err();

    assert_validation(
        err,
        "scope='cwd_inferred' writes require high confidence inference",
    );
    assert_eq!(store.export(None).await.unwrap().len(), 0);
    assert_eq!(store.list_scopes(None).await.unwrap().len(), scope_count);
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_rejects_empty_cwd_without_partial_write() {
    let (store, _dir) = common::test_store().await;
    common::create_global(&store).await;

    let err = deposit(
        &store,
        request(Some(ScopeSelector::cwd_inferred(Some("".into())))),
        &wctx(),
    )
    .await
    .unwrap_err();

    assert_validation(err, "cwd cannot be empty");
    assert_eq!(store.export(None).await.unwrap().len(), 0);
    assert_eq!(store.list_scopes(None).await.unwrap().len(), 1);
}
