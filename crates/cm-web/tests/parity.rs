//! Contract tests verifying semantic parity between agent web endpoints and MCP tools.
//!
//! Each test seeds a fixture store, calls both the web endpoint (via axum test client)
//! and the MCP tool handler (via cm-cli), then compares the shared response fields.
//! The `_trace` object is agent-endpoint-only and is excluded from comparison.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use cm_cli::mcp::tools;
use cm_core::{
    ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};
use cm_web::{AppState, api};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

// ── Helpers ──────────────────────────────────────────────────────

async fn test_store() -> (CmStore, tempfile::TempDir) {
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

fn test_app(store: CmStore) -> Router {
    let state = Arc::new(AppState { store });
    Router::new().nest("/api", api::router(state))
}

async fn get_json(app: Router, uri: &str) -> Value {
    let req = axum::http::Request::builder()
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200, "GET {uri} returned {}", resp.status());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn seed_entries(store: &CmStore) {
    ensure_scope(store, "global/project:helioy/repo:cm").await;

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

// ── Recall parity tests ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn recall_basic_query_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    // MCP tool
    let mcp_text = tools::cx_recall(&store, &json!({"query": "architecture"}))
        .await
        .unwrap();
    let mcp: Value = serde_json::from_str(&mcp_text).unwrap();

    // Web endpoint
    let app = test_app(store);
    let web = get_json(app, "/api/agent/recall?query=architecture").await;

    // Shared fields must match
    assert_eq!(mcp["returned"], web["returned"]);
    assert_eq!(mcp["scope_chain"], web["scope_chain"]);
    assert_eq!(mcp["token_estimate"], web["token_estimate"]);
    assert_eq!(
        mcp["results"].as_array().unwrap().len(),
        web["results"].as_array().unwrap().len()
    );

    // Entry fields match (comparing id, kind, title, snippet for each)
    for (m, w) in mcp["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(web["results"].as_array().unwrap())
    {
        assert_eq!(m["id"], w["id"]);
        assert_eq!(m["kind"], w["kind"]);
        assert_eq!(m["title"], w["title"]);
        assert_eq!(m["snippet"], w["snippet"]);
        assert_eq!(m["scope_path"], w["scope_path"]);
    }

    // Web has _trace, MCP does not
    assert!(web.get("_trace").is_some());
    assert!(mcp.get("_trace").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_with_scope_and_tags_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let mcp_text = tools::cx_recall(
        &store,
        &json!({
            "scope": "global/project:helioy",
            "tags": ["architecture"]
        }),
    )
    .await
    .unwrap();
    let mcp: Value = serde_json::from_str(&mcp_text).unwrap();

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/agent/recall?scope=global/project:helioy&tags=architecture",
    )
    .await;

    assert_eq!(mcp["returned"], web["returned"]);
    assert_eq!(mcp["scope_chain"], web["scope_chain"]);

    for (m, w) in mcp["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(web["results"].as_array().unwrap())
    {
        assert_eq!(m["id"], w["id"]);
        assert_eq!(m["kind"], w["kind"]);
        assert_eq!(m["title"], w["title"]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_entries_compat_matches_agent() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);

    // Agent endpoint
    let agent = get_json(app.clone(), "/api/agent/recall?query=architecture").await;

    // Compatibility endpoint
    let compat = get_json(app, "/api/entries/recall?query=architecture").await;

    // Shared fields must match
    assert_eq!(agent["returned"], compat["returned"]);
    assert_eq!(agent["scope_chain"], compat["scope_chain"]);
    assert_eq!(agent["token_estimate"], compat["token_estimate"]);

    // Entry fields match
    for (a, c) in agent["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(compat["results"].as_array().unwrap())
    {
        assert_eq!(a["id"], c["id"]);
        assert_eq!(a["kind"], c["kind"]);
        assert_eq!(a["title"], c["title"]);
        assert_eq!(a["snippet"], c["snippet"]);
    }

    // Compat has no _trace
    assert!(compat.get("_trace").is_none());
}

// ── Browse parity tests ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_basic_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let mcp_text = tools::cx_browse(&store, &json!({})).await.unwrap();
    let mcp: Value = serde_json::from_str(&mcp_text).unwrap();

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse").await;

    assert_eq!(mcp["total"], web["total"]);
    assert_eq!(mcp["has_more"], web["has_more"]);
    assert_eq!(
        mcp["entries"].as_array().unwrap().len(),
        web["entries"].as_array().unwrap().len()
    );

    for (m, w) in mcp["entries"]
        .as_array()
        .unwrap()
        .iter()
        .zip(web["entries"].as_array().unwrap())
    {
        assert_eq!(m["id"], w["id"]);
        assert_eq!(m["kind"], w["kind"]);
        assert_eq!(m["title"], w["title"]);
        assert_eq!(m["snippet"], w["snippet"]);
        assert_eq!(m["scope_path"], w["scope_path"]);
        assert_eq!(m["created_at"], w["created_at"]);
        assert_eq!(m["updated_at"], w["updated_at"]);
    }

    assert!(web.get("_trace").is_some());
    assert!(mcp.get("_trace").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_with_filters_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let mcp_text = tools::cx_browse(&store, &json!({"kind": "fact"}))
        .await
        .unwrap();
    let mcp: Value = serde_json::from_str(&mcp_text).unwrap();

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?kind=fact").await;

    assert_eq!(mcp["total"], web["total"]);
    for (m, w) in mcp["entries"]
        .as_array()
        .unwrap()
        .iter()
        .zip(web["entries"].as_array().unwrap())
    {
        assert_eq!(m["id"], w["id"]);
        assert_eq!(m["kind"], w["kind"]);
        assert_eq!(w["kind"], "fact");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_pagination_parity() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    seed_entries(&store).await;

    // Page 1
    let mcp_text = tools::cx_browse(&store, &json!({"limit": 2}))
        .await
        .unwrap();
    let mcp: Value = serde_json::from_str(&mcp_text).unwrap();

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?limit=2").await;

    assert_eq!(mcp["has_more"], web["has_more"]);
    assert_eq!(mcp["entries"].as_array().unwrap().len(), 2);
    assert_eq!(web["entries"].as_array().unwrap().len(), 2);

    // Both should have next_cursor
    assert!(mcp["next_cursor"].is_string());
    assert!(web["next_cursor"].is_string());

    // Page 2: create a second store from the same db to avoid clone
    let (write_pool2, read_pool2) = schema::create_pools(&db_path).await.unwrap();
    let store2 = CmStore::new(write_pool2, read_pool2);
    let cursor = web["next_cursor"].as_str().unwrap();
    let app2 = test_app(store2);
    let web2 = get_json(app2, &format!("/api/agent/browse?limit=2&cursor={cursor}")).await;
    assert_eq!(web2["entries"].as_array().unwrap().len(), 2);
}
