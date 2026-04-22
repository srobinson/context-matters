//! Contract tests verifying semantic parity between cm-web HTTP endpoints
//! and the underlying capability layer.
//!
//! Each test seeds a fixture store, calls both the web endpoint (via the
//! axum test client) and the capability layer directly (the same path the
//! MCP tools use), projects the capability result through the same
//! `project_web_*` helpers the web handlers use, then asserts full JSON
//! equality. If the two shapes ever drift, these tests catch it.

use std::{path::PathBuf, sync::Arc};

use axum::Router;
use axum::body::Body;
use cm_capabilities::browse::{self, BrowseRequest};
use cm_capabilities::projection::{project_web_browse, project_web_recall};
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

/// Round-trip a serialisable view through `to_vec`/`from_slice` so the
/// expected `Value` is built via the exact same path axum uses when
/// emitting a `Json(...)` response. `serde_json::to_value` would cast
/// `f32` fields to `f64` at full bit precision, while axum serialises
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
async fn capability_recall(store: &CmStore, request: RecallRequest) -> Value {
    let request_for_projection = request.clone();
    let result = recall::recall(store, request).await.unwrap();
    round_trip(&project_web_recall(&result, &request_for_projection))
}

/// Build the expected `WebBrowseView` JSON by driving the capability
/// layer directly and projecting through the same `project_web_browse`
/// helper the web handler uses.
async fn capability_browse(store: &CmStore, request: BrowseRequest) -> Value {
    let result = browse::browse(store, request).await.unwrap();
    round_trip(&project_web_browse(&result))
}

// ── Recall parity tests ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn recall_basic_query_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_recall(
        &store,
        RecallRequest {
            query: Some("architecture".to_owned()),
            limit: clamp_limit(None),
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/recall?query=architecture").await;

    assert_eq!(
        expected, web,
        "WebRecallView must match between capability layer and web endpoint"
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

    assert_eq!(
        expected, web,
        "Scoped+tagged recall must match capability layer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_entries_compat_matches_agent() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);

    let agent = get_json(app.clone(), "/api/agent/recall?query=architecture").await;
    let compat = get_json(app, "/api/entries/recall?query=architecture").await;

    // Post-migration both endpoints project through the same
    // `project_web_recall` helper, so the responses must be byte-identical.
    assert_eq!(
        agent, compat,
        "Compatibility alias must match agent endpoint exactly"
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
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse").await;

    assert_eq!(
        expected, web,
        "WebBrowseView must match between capability layer and web endpoint"
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
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?kind=fact").await;

    assert_eq!(expected, web, "Filtered browse must match capability layer");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_agent_sort_matches_entries_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            limit: None,
            sort: BrowseSort::TitleAsc,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let agent = get_json(app.clone(), "/api/agent/browse?sort=title_asc").await;
    let entries = get_json(app, "/api/entries?sort=title_asc").await;

    assert_eq!(
        expected, agent,
        "Agent browse sort must match capability layer"
    );
    assert_eq!(
        agent, entries,
        "Agent browse sort must match entries endpoint"
    );
    assert_eq!(agent["header"]["sort_used"], json!("title asc"));
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_agent_auto_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some(PathBuf::from("/tmp/helioy/context-matters")),
            include_resolution: Some(true),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/agent/browse?scope=auto&cwd=/tmp/helioy/context-matters",
    )
    .await;

    assert_eq!(expected, web, "Agent auto browse must match capability");
    assert_eq!(
        web["resolution"]["resolved_scope"],
        json!("global/project:helioy/repo:context-matters")
    );
    assert_eq!(web["resolution"]["confidence"], json!("high"));
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_entries_auto_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some(PathBuf::from("/tmp/helioy/context-matters")),
            include_resolution: Some(true),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/entries?scope=auto&cwd=/tmp/helioy/context-matters",
    )
    .await;

    assert_eq!(expected, web, "Entries auto browse must match capability");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_path_exact_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope_path: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?scope_path=global/project:helioy").await;

    assert_eq!(expected, web, "scope_path browse must stay exact");
    assert!(
        web.get("resolution").is_none(),
        "Explicit scope_path should not expose auto resolution"
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

    let cap_page1 = capability_browse(
        &store,
        BrowseRequest {
            scope_path: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: Some(1),
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web_page1 = get_json(
        app,
        "/api/agent/browse?scope_path=global/project:helioy&limit=1",
    )
    .await;

    assert_eq!(cap_page1, web_page1, "Page 1 must match capability layer");
    assert!(
        cap_page1["has_more"].as_bool().unwrap(),
        "Should have more pages"
    );
    assert!(
        cap_page1["next_cursor"].is_string(),
        "Capability must return next_cursor"
    );
    assert_eq!(
        cap_page1["next_cursor"], web_page1["next_cursor"],
        "Cursor values must match between capability and web"
    );

    let cursor = web_page1["next_cursor"].as_str().unwrap();
    let (write_pool2, read_pool2) = schema::create_pools(&db_path).await.unwrap();
    let store2 = CmStore::new(write_pool2, read_pool2);

    let cap_page2 = capability_browse(
        &store2,
        BrowseRequest {
            scope_path: Some(ScopePath::parse("global/project:helioy").unwrap()),
            limit: Some(1),
            sort: BrowseSort::Recent,
            cursor: Some(cursor.to_owned()),
            ..Default::default()
        },
    )
    .await;

    let (write_pool3, read_pool3) = schema::create_pools(&db_path).await.unwrap();
    let store3 = CmStore::new(write_pool3, read_pool3);
    let app2 = test_app(store3);
    let web_page2 = get_json(
        app2,
        &format!("/api/agent/browse?scope_path=global/project:helioy&limit=1&cursor={cursor}"),
    )
    .await;

    assert_eq!(cap_page2, web_page2, "Page 2 must match capability layer");
    assert_eq!(cap_page2["entries"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_auto_scope_pagination_parity() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    seed_entries(&store).await;

    let cap_page1 = capability_browse(
        &store,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some(PathBuf::from("/tmp/helioy/context-matters")),
            include_resolution: Some(true),
            limit: Some(1),
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let web_page1 = get_json(
        app,
        "/api/agent/browse?scope=auto&cwd=/tmp/helioy/context-matters&limit=1",
    )
    .await;

    assert_eq!(
        cap_page1, web_page1,
        "Auto browse page 1 must match capability layer"
    );
    assert!(
        cap_page1["has_more"].as_bool().unwrap(),
        "Auto browse should have a second page"
    );

    let cursor = web_page1["next_cursor"].as_str().unwrap();
    let (write_pool2, read_pool2) = schema::create_pools(&db_path).await.unwrap();
    let store2 = CmStore::new(write_pool2, read_pool2);

    let cap_page2 = capability_browse(
        &store2,
        BrowseRequest {
            scope: Some("auto".to_owned()),
            cwd: Some(PathBuf::from("/tmp/helioy/context-matters")),
            include_resolution: Some(true),
            limit: Some(1),
            sort: BrowseSort::Recent,
            cursor: Some(cursor.to_owned()),
            ..Default::default()
        },
    )
    .await;

    let (write_pool3, read_pool3) = schema::create_pools(&db_path).await.unwrap();
    let store3 = CmStore::new(write_pool3, read_pool3);
    let app2 = test_app(store3);
    let web_page2 = get_json(
        app2,
        &format!(
            "/api/agent/browse?scope=auto&cwd=/tmp/helioy/context-matters&limit=1&cursor={cursor}"
        ),
    )
    .await;

    assert_eq!(
        cap_page2, web_page2,
        "Auto browse page 2 must match capability layer"
    );
    assert_eq!(cap_page2["entries"].as_array().unwrap().len(), 1);
}

// ── Header contract tests ───────────────────────────────────────
//
// Pin the field names on the new projection shapes so a rename in
// `web_view.rs` cannot silently break the wire contract.

#[tokio::test(flavor = "multi_thread")]
async fn recall_header_has_required_fields() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let web = get_json(
        app,
        "/api/agent/recall?query=architecture&kinds=fact&tags=architecture",
    )
    .await;

    let header = web.get("header").expect("header must be present");
    for field in [
        "query",
        "routing",
        "candidates",
        "returned",
        "scope_chain",
        "scope_hits",
        "kinds_histogram",
        "tags_histogram",
        "tokens",
    ] {
        assert!(
            header.get(field).is_some(),
            "header.{field} required on WebRecallView"
        );
    }
    assert_eq!(header["query"], json!("architecture"));

    assert!(web.get("entries").is_some(), "entries array required");
    assert!(web.get("advisories").is_some(), "advisories array required");
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_header_has_required_fields() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let web = get_json(app, "/api/agent/browse?kind=fact&tag=architecture").await;

    let header = web.get("header").expect("header must be present");
    for field in [
        "sort_used",
        "total",
        "returned",
        "kinds_histogram",
        "tags_histogram",
    ] {
        assert!(
            header.get(field).is_some(),
            "header.{field} required on WebBrowseView"
        );
    }
    assert_eq!(header["sort_used"], json!("updated_at desc"));

    assert!(web.get("entries").is_some(), "entries array required");
    assert!(web.get("has_more").is_some(), "has_more flag required");
}
