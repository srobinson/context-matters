use cm_capabilities::browse::BrowseRequest;
use cm_capabilities::scope::ScopeSelector;
use cm_core::{BrowseSort, ScopePath};
use cm_store::{CmStore, schema};

use super::support::{
    capability_browse, cwd_inferred_scope_query, get_json, path_scope_query, seed_entries, test_app,
};

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
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
            limit: Some(1),
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = path_scope_query("global/project:helioy");
    let web_page1 = get_json(app, &format!("/api/agent/browse?{scope}&limit=1")).await;

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
            scope: Some(ScopeSelector::Path(
                ScopePath::parse("global/project:helioy").unwrap(),
            )),
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
        &format!("/api/agent/browse?{scope}&limit=1&cursor={cursor}"),
    )
    .await;

    assert_eq!(cap_page2, web_page2, "Page 2 must match capability layer");
    assert_eq!(cap_page2["entries"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_cwd_inferred_scope_pagination_parity() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    seed_entries(&store).await;

    let cap_page1 = capability_browse(
        &store,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            include_resolution: Some(true),
            limit: Some(1),
            sort: BrowseSort::Recent,
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let web_page1 = get_json(app, &format!("/api/agent/browse?{scope}&limit=1")).await;

    assert_eq!(
        cap_page1, web_page1,
        "cwd_inferred browse page 1 must match capability layer"
    );
    assert!(
        cap_page1["has_more"].as_bool().unwrap(),
        "cwd_inferred browse should have a second page"
    );

    let cursor = web_page1["next_cursor"].as_str().unwrap();
    let (write_pool2, read_pool2) = schema::create_pools(&db_path).await.unwrap();
    let store2 = CmStore::new(write_pool2, read_pool2);

    let cap_page2 = capability_browse(
        &store2,
        BrowseRequest {
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
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
        &format!("/api/agent/browse?{scope}&limit=1&cursor={cursor}"),
    )
    .await;

    assert_eq!(
        cap_page2, web_page2,
        "cwd_inferred browse page 2 must match capability layer"
    );
    assert_eq!(cap_page2["entries"].as_array().unwrap().len(), 1);
}
