use axum::http::{Method, StatusCode};
use cm_capabilities::recall::RecallRequest;
use cm_capabilities::scope::ScopeSelector;
use cm_core::{ContextStore, ScopePath};
use serde_json::json;

use super::support::{
    capability_recall, cwd_inferred_scope_query, path_scope_query, path_scope_value, request_json,
    seed_entries, test_app, test_store,
};

#[tokio::test(flavor = "multi_thread")]
async fn entries_search_uses_exact_scope_param() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let scope = ScopePath::parse("global/project:helioy/repo:context-matters").unwrap();
    let expected = capability_recall(
        &store,
        RecallRequest {
            query: Some("Smart".to_owned()),
            scope: Some(ScopeSelector::Path(scope)),
            kinds: Vec::new(),
            tags: Vec::new(),
            limit: 20,
            max_tokens: None,
        },
    )
    .await;

    let app = test_app(store);
    let scope = path_scope_query("global/project:helioy/repo:context-matters");
    let (status, web) = request_json(
        app,
        Method::GET,
        &format!("/api/entries/search?query=Smart&{scope}"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(expected, web);
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_search_uses_cwd_inferred_scope_param() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_recall(
        &store,
        RecallRequest {
            query: Some("Smart".to_owned()),
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            kinds: Vec::new(),
            tags: Vec::new(),
            limit: 20,
            max_tokens: None,
        },
    )
    .await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let (status, web) = request_json(
        app,
        Method::GET,
        &format!("/api/entries/search?query=Smart&{scope}"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(expected, web);
}

#[tokio::test(flavor = "multi_thread")]
async fn export_uses_exact_scope_param() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let scope = ScopePath::parse("global/project:helioy").unwrap();
    let expected = store.export(Some(&scope)).await.unwrap();

    let app = test_app(store);
    let scope = path_scope_query("global/project:helioy");
    let (status, web) = request_json(app, Method::GET, &format!("/api/export?{scope}"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(web, serde_json::to_value(expected).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn export_uses_cwd_inferred_scope_param() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let scope = ScopePath::parse("global/project:helioy/repo:context-matters").unwrap();
    let expected = store.export(Some(&scope)).await.unwrap();

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let (status, web) = request_json(app, Method::GET, &format!("/api/export?{scope}"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(web, serde_json::to_value(expected).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn cm_web_rejects_removed_scope_query_fields() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;
    let app = test_app(store);

    for uri in [
        "/api/entries/search?query=Smart&scope_path=global",
        "/api/entries/search?query=Smart&scope=global&scope_path=global",
        "/api/entries/search?query=Smart&scope_mode=resolved",
        "/api/entries/search?query=Smart&scope=auto",
        "/api/entries/recall?query=Smart&scope_path=global",
        "/api/entries/recall?query=Smart&scope_mode=resolved",
        "/api/entries/recall?query=Smart&scope=auto",
        "/api/agent/recall?query=Smart&scope_path=global",
        "/api/agent/recall?query=Smart&scope_mode=resolved",
        "/api/agent/recall?query=Smart&scope=auto",
        "/api/agent/browse?scope_path=global",
        "/api/agent/browse?scope_mode=resolved",
        "/api/agent/browse?scope=auto",
        "/api/entries?scope_path=global",
        "/api/entries?scope_mode=resolved",
        "/api/entries?scope=auto",
        "/api/export?scope_path=global",
        "/api/export?scope=global&scope_path=global",
        "/api/export?scope_mode=resolved",
        "/api/export?scope=auto",
    ] {
        let (status, body) = request_json(app.clone(), Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} returned {body:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn create_entry_body_uses_scope_not_scope_path() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let scope = path_scope_value("global/project:helioy");
    let body = json!({
        "scope": scope,
        "kind": "fact",
        "title": "Web create scope contract",
        "body": "Create bodies use scope as the public selector.",
        "created_by": "agent:test",
        "meta": { "tags": ["scope"] }
    });

    let (status, created) = request_json(app, Method::POST, "/api/entries", Some(body)).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["scope_path"], json!("global/project:helioy"));
}

#[tokio::test(flavor = "multi_thread")]
async fn merge_entry_body_uses_scope_not_scope_path() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;
    let entries = store.export(None).await.unwrap();
    let old_id = entries[0].id.to_string();

    let app = test_app(store);
    let scope = path_scope_value("global/project:helioy");
    let body = json!({
        "old_id": old_id,
        "new_entry": {
            "scope": scope,
            "kind": "fact",
            "title": "Web merge scope contract",
            "body": "Merge bodies use scope as the public selector.",
            "created_by": "agent:test",
            "meta": { "tags": ["scope"] }
        }
    });

    let (status, created) = request_json(app, Method::POST, "/api/entries/merge", Some(body)).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["scope_path"], json!("global/project:helioy"));
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_write_bodies_reject_removed_scope_inputs() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;
    let entries = store.export(None).await.unwrap();
    let old_id = entries[0].id.to_string();

    let app = test_app(store);
    let invalid_entries = [
        json!({
            "scope_path": "global/project:helioy",
            "kind": "fact",
            "title": "Removed scope path",
            "body": "This request should fail.",
            "created_by": "agent:test"
        }),
        json!({
            "scope": "auto",
            "kind": "fact",
            "title": "Removed auto",
            "body": "This request should fail.",
            "created_by": "agent:test"
        }),
        json!({
            "scope": path_scope_value("global/project:helioy"),
            "scope_mode": "resolved",
            "kind": "fact",
            "title": "Removed scope mode",
            "body": "This request should fail.",
            "created_by": "agent:test"
        }),
    ];

    for new_entry in invalid_entries {
        for (uri, body) in [
            ("/api/entries", new_entry.clone()),
            (
                "/api/entries/merge",
                json!({ "old_id": old_id, "new_entry": new_entry.clone() }),
            ),
        ] {
            let (status, body) = request_json(app.clone(), Method::POST, uri, Some(body)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} returned {body:?}");
        }
    }
}
