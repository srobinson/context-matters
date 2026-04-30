use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, StatusCode};
use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{project_web_browse, project_web_recall};
use cm_capabilities::recall::{self, RecallRequest};
use cm_core::{
    ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};
use cm_web::{AppState, api};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use url::form_urlencoded;

pub(super) async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    (CmStore::new(write_pool, read_pool), dir)
}

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

async fn ensure_scope(store: &CmStore, path: &str) {
    let sp = ScopePath::parse(path).unwrap();
    let ancestors: Vec<&str> = sp.ancestors().collect();
    for ancestor in ancestors.into_iter().rev() {
        let ancestor_path = ScopePath::parse(ancestor).unwrap();
        if store.get_scope(&ancestor_path).await.is_err() {
            let label = ancestor.rsplit('/').next().unwrap_or(ancestor);
            store
                .create_scope(
                    NewScope {
                        path: ancestor_path,
                        label: label.to_owned(),
                        meta: None,
                    },
                    &wctx(),
                )
                .await
                .unwrap();
        }
    }
}

pub(super) fn test_app(store: CmStore) -> Router {
    let state = Arc::new(AppState { store });
    Router::new().nest("/api", api::router(state))
}

pub(super) async fn get_json(app: Router, uri: &str) -> Value {
    let req = axum::http::Request::builder()
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200, "GET {uri} returned {}", resp.status());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

pub(super) fn scope_query(value: Value) -> String {
    let encoded: String = form_urlencoded::byte_serialize(value.to_string().as_bytes()).collect();
    format!("scope={encoded}")
}

pub(super) fn path_scope_value(path: &str) -> String {
    serde_json::json!({ "kind": "path", "path": path }).to_string()
}

pub(super) fn path_scope_query(path: &str) -> String {
    scope_query(serde_json::json!({ "kind": "path", "path": path }))
}

pub(super) fn cwd_inferred_scope_query(cwd: &str) -> String {
    scope_query(serde_json::json!({ "kind": "cwd_inferred", "cwd": cwd }))
}

pub(super) async fn request_json(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = axum::http::Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let req = builder
        .body(match body {
            Some(value) => Body::from(serde_json::to_vec(&value).unwrap()),
            None => Body::empty(),
        })
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, json)
}

pub(super) async fn seed_entries(store: &CmStore) {
    ensure_scope(store, "global/project:helioy/repo:cm").await;
    ensure_scope(store, "global/project:helioy/repo:context-matters").await;

    let entries = vec![
        (
            "Architecture overview",
            "The system uses a layered architecture with core, store, capabilities, and adapter crates.",
            "fact",
            "global",
            None,
        ),
        (
            "SQLite for storage",
            "We use SQLite with WAL mode for the context store.",
            "decision",
            "global/project:helioy",
            Some(vec!["architecture", "database"]),
        ),
        (
            "Scope hierarchy design",
            "Scopes form a tree: global > project > repo > session.",
            "fact",
            "global/project:helioy/repo:cm",
            Some(vec!["architecture"]),
        ),
        (
            "Smart browse local scope",
            "Auto browse should resolve the local repo scope from cwd before building the exact filter.",
            "decision",
            "global/project:helioy/repo:context-matters",
            Some(vec!["smart-browse", "scope"]),
        ),
        (
            "Smart browse pagination",
            "Cursor pagination should operate after auto scope resolution chooses the exact repo filter.",
            "fact",
            "global/project:helioy/repo:context-matters",
            Some(vec!["smart-browse", "pagination"]),
        ),
        (
            "Prefer capability delegation",
            "MCP and web endpoints should delegate to shared capability functions.",
            "lesson",
            "global/project:helioy",
            Some(vec!["refactor", "architecture"]),
        ),
    ];

    for (title, body, kind, scope, tags) in entries {
        let meta = tags.map(|t: Vec<&str>| EntryMeta {
            tags: t.into_iter().map(String::from).collect(),
            ..Default::default()
        });
        store
            .create_entry(
                NewEntry {
                    scope_path: ScopePath::parse(scope).unwrap(),
                    kind: kind.parse::<EntryKind>().unwrap(),
                    title: title.to_owned(),
                    body: body.to_owned(),
                    created_by: "agent:test".to_owned(),
                    meta,
                },
                &wctx(),
            )
            .await
            .unwrap();
    }
}

/// Round trip a serializable view through `to_vec` and `from_slice` so
/// the expected `Value` is built via the exact same path axum uses when
/// emitting a `Json(...)` response. `serde_json::to_value` would cast
/// `f32` fields to `f64` at full bit precision, while axum serializes
/// f32 via Ryu's shortest representation. The two paths produce
/// numerically distinct `Number`s for the BM25 score column, so a
/// direct `to_value` would flake the parity assertions.
fn round_trip<T: serde::Serialize>(value: &T) -> Value {
    let bytes = serde_json::to_vec(value).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Build the expected `WebRecallView` JSON by driving the capability
/// layer directly and projecting through the same `project_web_recall`
/// helper the web handler uses. The request is cloned so it can be
/// passed to the projection alongside the result.
pub(super) async fn capability_recall(store: &CmStore, request: RecallRequest) -> Value {
    let request_for_projection = request.clone();
    let result = recall::recall(store, request).await.unwrap();
    round_trip(&project_web_recall(&result, &request_for_projection))
}

/// Build the expected `WebBrowseView` JSON by driving the capability
/// layer directly and projecting through the same `project_web_browse`
/// helper the web handler uses.
pub(super) async fn capability_browse(store: &CmStore, request: BrowseRequest) -> Value {
    let result = browse::browse(store, request).await.unwrap();
    round_trip(&project_web_browse(&result))
}
