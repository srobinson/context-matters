use cm_capabilities::browse::BrowseRequest;
use cm_capabilities::scope::ScopeSelector;
use cm_core::{BrowseSort, EntryKind, ScopePath};
use serde_json::json;

use super::support::{
    all_scope_query, capability_browse, cwd_inferred_scope_query, get_json, path_scope_query,
    seed_entries, set_scope_query, subtree_scope_query, test_app, test_store,
};

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
async fn browse_agent_cwd_inferred_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            include_resolution: Some(true),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let web = get_json(app, &format!("/api/agent/browse?{scope}")).await;

    assert_eq!(
        expected, web,
        "Agent cwd_inferred browse must match capability"
    );
    assert_eq!(
        web["resolution"]["resolved_scope"],
        json!("global/project:helioy/repo:context-matters")
    );
    assert_eq!(web["resolution"]["confidence"], json!("high"));
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_entries_cwd_inferred_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            include_resolution: Some(true),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let web = get_json(app, &format!("/api/entries?{scope}")).await;

    assert_eq!(
        expected, web,
        "Entries cwd_inferred browse must match capability"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_scope_exact_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = path_scope_query("global/project:helioy");
    let web = get_json(app, &format!("/api/agent/browse?{scope}")).await;

    assert_eq!(expected, web, "scope browse must stay exact");
    assert!(
        web.get("resolution").is_none(),
        "Explicit scope should not expose resolution"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_subtree_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::Subtree(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = subtree_scope_query("global/project:helioy");
    let agent = get_json(app.clone(), &format!("/api/agent/browse?{scope}")).await;
    let entries = get_json(app, &format!("/api/entries?{scope}")).await;

    assert_eq!(
        expected, agent,
        "Agent subtree browse must match capability"
    );
    assert_eq!(agent, entries, "Entries subtree browse must match agent");
    assert!(agent.get("resolution").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_set_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::Set(vec![
                ScopePath::parse("global").unwrap(),
                ScopePath::parse("global/project:helioy/repo:cm").unwrap(),
            ])),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = set_scope_query(&["global", "global/project:helioy/repo:cm"]);
    let agent = get_json(app.clone(), &format!("/api/agent/browse?{scope}")).await;
    let entries = get_json(app, &format!("/api/entries?{scope}")).await;

    assert_eq!(expected, agent, "Agent set browse must match capability");
    assert_eq!(agent, entries, "Entries set browse must match agent");
    assert!(agent.get("resolution").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_all_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::All),
            limit: None,
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = all_scope_query();
    let agent = get_json(app.clone(), &format!("/api/agent/browse?{scope}")).await;
    let entries = get_json(app, &format!("/api/entries?{scope}")).await;

    assert_eq!(expected, agent, "Agent all browse must match capability");
    assert_eq!(agent, entries, "Entries all browse must match agent");
    assert!(agent.get("resolution").is_none());
}
