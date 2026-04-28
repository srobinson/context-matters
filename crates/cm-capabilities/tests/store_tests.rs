//! Capability-level tests for store defaults, validation, metadata parsing,
//! scope creation, and supersedes handling.

mod common;

use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::store::{StoreRequest, store as store_entry};
use cm_core::{
    CmError, Confidence, ContextStore, MutationSource, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};
use serde_json::json;

async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::global(),
                label: "Global".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

fn request(value: serde_json::Value) -> StoreRequest {
    serde_json::from_value(value).unwrap()
}

fn minimal_request(title: &str, body: &str) -> StoreRequest {
    request(json!({
        "title": title,
        "body": body,
        "kind": "fact"
    }))
}

fn assert_validation(err: CmError, expected: &str) {
    match err {
        CmError::Validation(msg) => assert_eq!(msg, expected),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn store_request_deserializes_scope_and_created_by_defaults() {
    let request = minimal_request("Defaulted store", "Body.");

    assert_eq!(request.scope, None);
    assert_eq!(request.created_by, "agent:claude-code");
    assert!(request.meta.is_empty());
}

#[test]
fn store_request_deserializes_exact_scope_selector() {
    let request = request(json!({
        "title": "Scoped store",
        "body": "Body.",
        "kind": "fact",
        "scope": "global/project:helioy/repo:context-matters"
    }));

    assert_eq!(
        request.scope,
        Some(ScopeSelector::Path(
            ScopePath::parse("global/project:helioy/repo:context-matters").unwrap()
        ))
    );
}

#[test]
fn store_request_rejects_removed_scope_path_input() {
    let err = serde_json::from_value::<StoreRequest>(json!({
        "title": "Legacy store",
        "body": "Body.",
        "kind": "fact",
        "scope_path": "global/project:helioy"
    }))
    .unwrap_err();

    assert!(
        err.to_string().contains("scope_path"),
        "unexpected error: {err}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_oversized_body_before_writing() {
    let (store, _dir) = test_store().await;
    let mut request = minimal_request("Large body", "Body.");
    request.body = "x".repeat(cm_capabilities::constants::MAX_INPUT_BYTES + 1);

    let err = store_entry(&store, request, &wctx()).await.unwrap_err();

    assert_validation(
        err,
        &format!(
            "body exceeds {} byte limit",
            cm_capabilities::constants::MAX_INPUT_BYTES
        ),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_invalid_confidence_through_meta_input() {
    let (store, _dir) = test_store().await;
    let request = request(json!({
        "title": "Bad confidence",
        "body": "Body.",
        "kind": "fact",
        "confidence": "certain"
    }));

    let err = store_entry(&store, request, &wctx()).await.unwrap_err();

    assert_validation(
        err,
        "Invalid confidence 'certain'. Valid values: high, medium, low.",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn store_persists_metadata_via_meta_input() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let request = request(json!({
        "title": "Metadata",
        "body": "Body.",
        "kind": "fact",
        "tags": ["dx", "adapter"],
        "confidence": "high",
        "source": "https://example.test/context",
        "priority": 7
    }));

    let result = store_entry(&store, request, &wctx()).await.unwrap();
    let entry = store
        .get_entry(uuid::Uuid::parse_str(&result.entry_id).unwrap())
        .await
        .unwrap();
    let meta = entry.meta.unwrap();

    assert_eq!(meta.tags, vec!["dx", "adapter"]);
    assert_eq!(meta.confidence, Some(Confidence::High));
    assert_eq!(meta.source.as_deref(), Some("https://example.test/context"));
    assert_eq!(meta.priority, Some(7));
}

#[tokio::test(flavor = "multi_thread")]
async fn store_auto_creates_scope_chain_and_reports_creation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let request = request(json!({
        "title": "Repo decision",
        "body": "Use sqlx.",
        "kind": "decision",
        "scope": "global/project:helioy/repo:nancyr"
    }));

    let result = store_entry(&store, request, &wctx()).await.unwrap();

    assert!(result.scope_created);
    assert_eq!(result.scope_path, "global/project:helioy/repo:nancyr");

    let project = store
        .get_scope(&ScopePath::parse("global/project:helioy").unwrap())
        .await
        .unwrap();
    assert_eq!(project.label, "helioy");

    let repo = store
        .get_scope(&ScopePath::parse("global/project:helioy/repo:nancyr").unwrap())
        .await
        .unwrap();
    assert_eq!(repo.label, "nancyr");
}

#[tokio::test(flavor = "multi_thread")]
async fn store_resolves_cwd_inferred_scope_before_writing() {
    let (store, _dir) = test_store().await;
    common::ensure_scope(&store, "global/project:helioy/repo:context-matters").await;
    let mut request = minimal_request("Inferred write", "Body.");
    request.scope = Some(ScopeSelector::cwd_inferred(Some(
        "/tmp/helioy/context-matters".into(),
    )));

    let result = store_entry(&store, request, &wctx()).await.unwrap();

    assert!(!result.scope_created);
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
async fn store_rejects_low_confidence_cwd_inferred_without_partial_write() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let mut request = minimal_request("Rejected inferred write", "Body.");
    request.scope = Some(ScopeSelector::cwd_inferred(Some(
        "/tmp/acme/no-local-match".into(),
    )));

    let err = store_entry(&store, request, &wctx()).await.unwrap_err();

    assert_validation(
        err,
        "scope='cwd_inferred' writes require high confidence inference",
    );
    assert_eq!(store.export(None).await.unwrap().len(), 0);
    assert_eq!(store.list_scopes(None).await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_ambiguous_cwd_inferred_without_partial_write() {
    let (store, _dir) = test_store().await;
    common::ensure_scope(&store, "global/project:alpha/repo:context-matters").await;
    common::ensure_scope(&store, "global/project:beta/repo:context-matters").await;
    let scope_count = store.list_scopes(None).await.unwrap().len();
    let mut request = minimal_request("Rejected ambiguous write", "Body.");
    request.scope = Some(ScopeSelector::cwd_inferred(Some(
        "/tmp/worktrees/context-matters".into(),
    )));

    let err = store_entry(&store, request, &wctx()).await.unwrap_err();

    assert_validation(
        err,
        "scope='cwd_inferred' writes require high confidence inference",
    );
    assert_eq!(store.export(None).await.unwrap().len(), 0);
    assert_eq!(store.list_scopes(None).await.unwrap().len(), scope_count);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_reports_existing_scope_without_creation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = store_entry(&store, minimal_request("Existing", "Body."), &wctx())
        .await
        .unwrap();

    assert!(!result.scope_created);
    assert_eq!(result.scope_path, "global");
}

#[tokio::test(flavor = "multi_thread")]
async fn store_rejects_invalid_supersedes_with_existing_message() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    let mut request = minimal_request("Bad supersedes", "Body.");
    request.supersedes = Some("not-a-uuid".to_owned());

    let err = store_entry(&store, request, &wctx()).await.unwrap_err();

    assert_validation(err, "Invalid supersedes ID: 'not-a-uuid'. Expected a UUID.");
}
