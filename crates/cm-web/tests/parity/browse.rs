use std::path::PathBuf;

use cm_capabilities::browse::BrowseRequest;
use cm_core::{BrowseSort, EntryKind, ScopePath};
use serde_json::json;

use super::support::{capability_browse, get_json, seed_entries, test_app, test_store};

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
