use cm_capabilities::recall::RecallRequest;
use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::validation::clamp_limit;
use cm_core::ScopePath;

use super::support::{
    capability_recall, cwd_inferred_scope_query, get_json, path_scope_query, seed_entries,
    test_app, test_store,
};

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
            scope: Some(ScopeSelector::Path(scope)),
            tags: vec!["architecture".to_owned()],
            limit: clamp_limit(None),
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = path_scope_query("global/project:helioy");
    let web = get_json(app, &format!("/api/agent/recall?{scope}&tags=architecture")).await;

    assert_eq!(
        expected, web,
        "Scoped+tagged recall must match capability layer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_agent_cwd_inferred_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_recall(
        &store,
        RecallRequest {
            query: Some("Smart".to_owned()),
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            limit: clamp_limit(None),
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let web = get_json(app, &format!("/api/agent/recall?query=Smart&{scope}")).await;

    assert_eq!(
        expected, web,
        "Agent cwd_inferred recall must match capability layer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_entries_cwd_inferred_scope_parity() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let expected = capability_recall(
        &store,
        RecallRequest {
            query: Some("Smart".to_owned()),
            scope: Some(ScopeSelector::cwd_inferred(Some(
                "/tmp/helioy/context-matters".into(),
            ))),
            limit: clamp_limit(None),
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);
    let scope = cwd_inferred_scope_query("/tmp/helioy/context-matters");
    let web = get_json(app, &format!("/api/entries/recall?query=Smart&{scope}")).await;

    assert_eq!(
        expected, web,
        "Entries cwd_inferred recall must match capability layer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_entries_compat_matches_agent() {
    let (store, _dir) = test_store().await;
    seed_entries(&store).await;

    let app = test_app(store);

    let agent = get_json(app.clone(), "/api/agent/recall?query=architecture").await;
    let compat = get_json(app, "/api/entries/recall?query=architecture").await;

    // Post migration both endpoints project through the same
    // `project_web_recall` helper, so the responses must be byte identical.
    assert_eq!(
        agent, compat,
        "Compatibility alias must match agent endpoint exactly"
    );
}
