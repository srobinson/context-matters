//! Contract tests verifying semantic parity between agent web endpoints and MCP tools.
//!
//! Each test seeds a fixture store, calls both the web endpoint (via axum test client)
//! and the capability layer directly (the same path MCP tools use), then compares
//! the full shared response fields. The `_trace` object is agent-endpoint-only and is
//! excluded from comparison.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{project_browse_entry, project_recall_entry};
use cm_capabilities::recall::{self, RecallRequest};
use cm_capabilities::validation::clamp_limit;
use cm_core::{
    BrowseSort, ContextStore, EntryKind, EntryMeta, MutationSource, NewEntry, NewScope, ScopePath,
    WriteContext,
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

/// Strip the `_trace` key from a JSON value, returning the shared fields only.
fn strip_trace(mut v: Value) -> Value {
    if let Some(obj) = v.as_object_mut() {
        obj.remove("_trace");
    }
    v
}

/// Build the expected MCP-equivalent recall JSON by calling the capability layer directly.
async fn capability_recall(store: &CmStore, request: RecallRequest) -> Value {
    let result = recall::recall(store, request).await.unwrap();
    let results: Vec<Value> = result
        .entries
        .iter()
        .map(|e| serde_json::to_value(project_recall_entry(e)).unwrap())
        .collect();

    let scope_hits: std::collections::BTreeMap<String, usize> =
        result.scope_hits.iter().cloned().collect();

    json!({
        "results": results,
        "returned": results.len(),
        "scope_chain": result.scope_chain,
        "scope_hits": scope_hits,
        "token_estimate": result.token_estimate,
    })
}

/// Build the expected MCP-equivalent browse JSON by calling the capability layer directly.
async fn capability_browse(store: &CmStore, request: BrowseRequest) -> Value {
    let result = browse::browse(store, request).await.unwrap();
    let entries: Vec<Value> = result
        .entries
        .iter()
        .map(|e| serde_json::to_value(project_browse_entry(e)).unwrap())
        .collect();

    json!({
        "entries": entries,
        "total": result.total,
        "next_cursor": result.next_cursor,
        "has_more": result.has_more,
    })
}

// ── Recall parity tests ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn recall_basic_query_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    // Capability layer (same path MCP uses)
    let expected = capability_recall(
        &store,
        RecallRequest {
            query: Some("architecture".to_owned()),
            limit: clamp_limit(None),
            ..Default::default()
        },
    )
    .await;

    // Web endpoint
    let app = test_app(store);
    let web = get_json(app, "/api/agent/recall?query=architecture").await;
    let web_shared = strip_trace(web.clone());

    // Full shared-field equality
    assert_eq!(
        expected, web_shared,
        "Shared fields must match between capability layer and web endpoint"
    );

    // Web has _trace, capability does not
    assert!(
        web.get("_trace").is_some(),
        "Agent endpoint must include _trace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_with_scope_and_tags_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let scope = ScopePath::parse("global/project:helioy").unwrap();
    let expected = capability_recall(
        &store,
        RecallRequest {
            scope: Some(scope),
            tags: vec!["architecture".to_owned()],
            limit: clamp_limit(None),
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/agent/recall?scope=global/project:helioy&tags=architecture",
    )
    .await;
    let web_shared = strip_trace(web);

    assert_eq!(
        expected, web_shared,
        "Scoped+tagged recall must match capability layer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_entries_compat_matches_agent() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);

    // Agent endpoint (with _trace)
    let agent = get_json(app.clone(), "/api/agent/recall?query=architecture").await;
    let agent_shared = strip_trace(agent);

    // Compatibility endpoint (without _trace)
    let compat = get_json(app, "/api/entries/recall?query=architecture").await;

    // Must be identical on all shared fields
    assert_eq!(
        agent_shared, compat,
        "Compatibility alias must match agent endpoint on shared fields"
    );

    // Compat must not have _trace
    assert!(
        compat.get("_trace").is_none(),
        "Compatibility endpoint must not include _trace"
    );
}

// ── Browse parity tests ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn browse_basic_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            limit: clamp_limit(None),
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse").await;
    let web_shared = strip_trace(web.clone());

    assert_eq!(
        expected, web_shared,
        "Shared fields must match between capability layer and web endpoint"
    );
    assert!(
        web.get("_trace").is_some(),
        "Agent endpoint must include _trace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_with_filters_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            kind: Some(EntryKind::Fact),
            limit: clamp_limit(None),
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?kind=fact").await;
    let web_shared = strip_trace(web);

    assert_eq!(
        expected, web_shared,
        "Filtered browse must match capability layer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_pagination_parity() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    seed_entries(&store).await;

    // Page 1 via capability
    let cap_page1 = capability_browse(
        &store,
        BrowseRequest {
            limit: 2,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    // Page 1 via web
    let app = test_app(store);
    let web_page1 = get_json(app, "/api/agent/browse?limit=2").await;
    let web_page1_shared = strip_trace(web_page1.clone());

    assert_eq!(
        cap_page1, web_page1_shared,
        "Page 1 must match capability layer"
    );
    assert!(
        cap_page1["has_more"].as_bool().unwrap(),
        "Should have more pages"
    );
    assert!(
        cap_page1["next_cursor"].is_string(),
        "Capability must return next_cursor"
    );

    // Verify cursor values match
    assert_eq!(
        cap_page1["next_cursor"], web_page1_shared["next_cursor"],
        "Cursor values must match between capability and web"
    );

    // Page 2 via web using cursor from page 1
    let cursor = web_page1_shared["next_cursor"].as_str().unwrap();
    let (write_pool2, read_pool2) = schema::create_pools(&db_path).await.unwrap();
    let store2 = CmStore::new(write_pool2, read_pool2);

    let cap_page2 = capability_browse(
        &store2,
        BrowseRequest {
            limit: 2,
            sort: BrowseSort::Recent,
            cursor: Some(cursor.to_owned()),
            ..Default::default()
        },
    )
    .await;

    let (write_pool3, read_pool3) = schema::create_pools(&db_path).await.unwrap();
    let store3 = CmStore::new(write_pool3, read_pool3);
    let app2 = test_app(store3);
    let web_page2 = get_json(app2, &format!("/api/agent/browse?limit=2&cursor={cursor}")).await;
    let web_page2_shared = strip_trace(web_page2);

    assert_eq!(
        cap_page2, web_page2_shared,
        "Page 2 must match capability layer"
    );
    assert_eq!(cap_page2["entries"].as_array().unwrap().len(), 2);
}

// ── Trace contract tests ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn recall_trace_has_required_fields() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/agent/recall?query=architecture&kinds=fact&tags=architecture",
    )
    .await;
    let trace = web.get("_trace").expect("_trace must be present");

    assert!(trace.get("routing").is_some(), "_trace.routing required");
    assert!(
        trace.get("candidates_before_filter").is_some(),
        "_trace.candidates_before_filter required"
    );
    assert!(
        trace.get("fetch_limit_used").is_some(),
        "_trace.fetch_limit_used required"
    );
    assert!(
        trace.get("post_filters_applied").is_some(),
        "_trace.post_filters_applied required"
    );
    assert!(
        trace.get("token_budget_exhausted").is_some(),
        "_trace.token_budget_exhausted required"
    );

    let post_filters = trace["post_filters_applied"].as_array().unwrap();
    let filter_names: Vec<&str> = post_filters.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(
        filter_names.contains(&"kinds"),
        "post_filters_applied should include 'kinds'"
    );
    assert!(
        filter_names.contains(&"tags"),
        "post_filters_applied should include 'tags'"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_trace_has_structured_filter_set() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?kind=fact&tag=architecture").await;
    let trace = web.get("_trace").expect("_trace must be present");

    let filter_set = trace.get("filter_set").expect("_trace.filter_set required");
    assert!(
        filter_set.is_object(),
        "filter_set must be a structured object"
    );
    assert!(
        filter_set.get("scope_path").is_some(),
        "filter_set.scope_path required"
    );
    assert_eq!(filter_set["kind"], "fact");
    assert_eq!(filter_set["tag"], "architecture");
    assert_eq!(filter_set["include_superseded"], false);

    assert!(trace.get("sort").is_some(), "_trace.sort required");
    assert_eq!(trace["sort"], "recent");
}
