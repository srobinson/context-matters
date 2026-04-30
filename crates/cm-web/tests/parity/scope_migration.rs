use axum::http::{Method, StatusCode};
use cm_core::{ContextStore, ScopePath};
use serde_json::json;

use super::support::{
    all_scope_query, cwd_inferred_scope_query, path_scope_query, path_scope_value, request_json,
    seed_entries, set_scope_query, subtree_scope_query, test_app, test_store,
};

#[tokio::test(flavor = "multi_thread")]
async fn entries_search_uses_exact_scope_param() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let scope = path_scope_query("global/project:helioy/repo:context-matters");
    let entries_uri = format!("/api/entries/search?query=Smart&{scope}");
    let agent_uri = format!("/api/agent/search?query=Smart&{scope}");
    let (entries_status, entries_body) =
        request_json(app.clone(), Method::GET, &entries_uri, None).await;
    let (agent_status, agent_body) = request_json(app, Method::GET, &agent_uri, None).await;

    assert_eq!(entries_status, StatusCode::OK);
    assert_eq!(agent_status, StatusCode::OK);
    assert_eq!(entries_body, agent_body);
    assert_eq!(
        entries_body["header"]["scope_chain"],
        json!(["global/project:helioy/repo:context-matters"])
    );
    assert_eq!(entries_body["entries"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_search_uses_cwd_inferred_scope_param() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let entries_uri = format!("/api/entries/search?query=Smart&{scope}");
    let agent_uri = format!("/api/agent/search?query=Smart&{scope}");
    let (entries_status, entries_body) =
        request_json(app.clone(), Method::GET, &entries_uri, None).await;
    let (agent_status, agent_body) = request_json(app, Method::GET, &agent_uri, None).await;

    assert_eq!(entries_status, StatusCode::OK);
    assert_eq!(agent_status, StatusCode::OK);
    assert_eq!(entries_body, agent_body);
    assert_eq!(
        entries_body["header"]["scope_chain"],
        json!(["global/project:helioy/repo:context-matters"])
    );
    assert_eq!(entries_body["entries"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_and_agent_search_match_for_all_scope() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let scope = all_scope_query();
    let entries_uri = format!("/api/entries/search?query=Smart&{scope}");
    let agent_uri = format!("/api/agent/search?query=Smart&{scope}");
    let (entries_status, entries_body) =
        request_json(app.clone(), Method::GET, &entries_uri, None).await;
    let (agent_status, agent_body) = request_json(app, Method::GET, &agent_uri, None).await;

    assert_eq!(entries_status, StatusCode::OK);
    assert_eq!(agent_status, StatusCode::OK);
    assert_eq!(entries_body, agent_body);
    assert_eq!(entries_body["header"]["routing"], json!("search"));
    assert_eq!(entries_body["header"]["scope_chain"], json!([]));
    assert_eq!(entries_body["header"]["tier"], json!(null));
    assert_eq!(entries_body["entries"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_and_agent_search_accept_repeated_kind_and_tag_filters() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let scope = all_scope_query();
    let filters = "kind=decision&kind=fact&tag=scope&tag=pagination";
    let entries_uri = format!("/api/entries/search?query=Smart&{scope}&{filters}");
    let agent_uri = format!("/api/agent/search?query=Smart&{scope}&{filters}");
    let (entries_status, entries_body) =
        request_json(app.clone(), Method::GET, &entries_uri, None).await;
    let (agent_status, agent_body) = request_json(app, Method::GET, &agent_uri, None).await;

    assert_eq!(entries_status, StatusCode::OK);
    assert_eq!(agent_status, StatusCode::OK);
    assert_eq!(entries_body, agent_body);
    assert_eq!(
        entry_titles(&entries_body),
        vec!["Smart browse local scope", "Smart browse pagination"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_and_agent_search_reject_invalid_repeated_kind_filters() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let filters = "kind=unknown-kind&kind=fact";
    let entries_uri = format!("/api/entries/search?query=Smart&{filters}");
    let agent_uri = format!("/api/agent/search?query=Smart&{filters}");
    let (entries_status, entries_body) =
        request_json(app.clone(), Method::GET, &entries_uri, None).await;
    let (agent_status, agent_body) = request_json(app, Method::GET, &agent_uri, None).await;

    assert_eq!(entries_status, StatusCode::BAD_REQUEST);
    assert_eq!(agent_status, StatusCode::BAD_REQUEST);
    assert_eq!(entries_body, agent_body);
    assert!(entries_body.to_string().contains("unknown-kind"));
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_and_agent_search_reject_invalid_fts_queries() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    for query in ["AND", "Smart%20OR"] {
        let entries_uri = format!("/api/entries/search?query={query}");
        let agent_uri = format!("/api/agent/search?query={query}");
        let (entries_status, entries_body) =
            request_json(app.clone(), Method::GET, &entries_uri, None).await;
        let (agent_status, agent_body) =
            request_json(app.clone(), Method::GET, &agent_uri, None).await;

        assert_eq!(entries_status, StatusCode::BAD_REQUEST, "{entries_uri}");
        assert_eq!(agent_status, StatusCode::BAD_REQUEST, "{agent_uri}");
        assert_eq!(entries_body, agent_body);
        let body_text = entries_body.to_string();
        assert!(
            body_text.contains("invalid cx_search input"),
            "{query}: {body_text}"
        );
        assert!(
            body_text.contains("query is invalid"),
            "{query}: {body_text}"
        );
    }
}

fn entry_titles(body: &serde_json::Value) -> Vec<&str> {
    let mut titles: Vec<&str> = body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["title"].as_str().unwrap())
        .collect();
    titles.sort_unstable();
    titles
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_search_omits_scope_chain_for_wide_selectors() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);
    let cases = [
        subtree_scope_query("global/project:helioy"),
        set_scope_query(&["global", "global/project:helioy/repo:context-matters"]),
    ];

    for scope in cases {
        let entries_uri = format!("/api/entries/search?query=Smart&{scope}");
        let agent_uri = format!("/api/agent/search?query=Smart&{scope}");
        let (entries_status, entries_body) =
            request_json(app.clone(), Method::GET, &entries_uri, None).await;
        let (agent_status, agent_body) =
            request_json(app.clone(), Method::GET, &agent_uri, None).await;

        assert_eq!(entries_status, StatusCode::OK, "{entries_uri}");
        assert_eq!(agent_status, StatusCode::OK, "{agent_uri}");
        assert_eq!(entries_body, agent_body, "parity drift on {scope}");
        assert_eq!(
            entries_body["header"]["scope_chain"],
            json!([]),
            "wide selector should not advertise an ancestor chain on {scope}"
        );
        assert_eq!(entries_body["header"]["routing"], json!("search"));
        assert_eq!(entries_body["header"]["tier"], json!(null));
    }
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
        "/api/entries/search?query=Smart&cwd=/tmp/helioy/context-matters",
        "/api/entries/search?query=Smart&scope=auto",
        "/api/entries/search?query=Smart&cursor=abc",
        "/api/entries/recall?query=Smart&scope_path=global",
        "/api/entries/recall?query=Smart&scope_mode=resolved",
        "/api/entries/recall?query=Smart&cwd=/tmp/helioy/context-matters",
        "/api/entries/recall?query=Smart&scope=auto",
        "/api/agent/recall?query=Smart&scope_path=global",
        "/api/agent/recall?query=Smart&scope_mode=resolved",
        "/api/agent/recall?query=Smart&cwd=/tmp/helioy/context-matters",
        "/api/agent/recall?query=Smart&scope=auto",
        "/api/agent/search?query=Smart&scope_path=global",
        "/api/agent/search?query=Smart&scope_mode=resolved",
        "/api/agent/search?query=Smart&cwd=/tmp/helioy/context-matters",
        "/api/agent/search?query=Smart&scope=auto",
        "/api/agent/search?query=Smart&cursor=abc",
        "/api/agent/browse?scope_path=global",
        "/api/agent/browse?scope_mode=resolved",
        "/api/agent/browse?cwd=/tmp/helioy/context-matters",
        "/api/agent/browse?scope=auto",
        "/api/entries?scope_path=global",
        "/api/entries?scope_mode=resolved",
        "/api/entries?cwd=/tmp/helioy/context-matters",
        "/api/entries?scope=auto",
        "/api/export?scope_path=global",
        "/api/export?scope=global&scope_path=global",
        "/api/export?scope_mode=resolved",
        "/api/export?cwd=/tmp/helioy/context-matters",
        "/api/export?scope=auto",
    ] {
        let (status, body) = request_json(app.clone(), Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} returned {body:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cm_web_reports_invalid_structured_scope_json() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;
    let app = test_app(store);

    for uri in [
        "/api/agent/recall?query=Smart&scope=%7Bbad",
        "/api/agent/search?query=Smart&scope=%7Bbad",
        "/api/agent/browse?scope=%7Bbad",
        "/api/entries/recall?query=Smart&scope=%7Bbad",
        "/api/entries/search?query=Smart&scope=%7Bbad",
        "/api/entries?scope=%7Bbad",
        "/api/export?scope=%7Bbad",
    ] {
        let (status, body) = request_json(app.clone(), Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} returned {body:?}");
        assert!(
            body.to_string()
                .contains("scope must be structured JSON with a kind field"),
            "{uri} returned {body:?}"
        );
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
