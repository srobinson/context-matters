use cm_core::{
    ContextStore, RecallShadowListFilter, RecallShadowPositionDelta, RecallShadowRecord,
    RecallShadowResponse,
};
use cm_store::CmStore;
use serde_json::Value;
use uuid::Uuid;

use super::support::{get_json, test_app, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn recall_shadow_list_and_filter_parity() {
    let (store, _dir) = test_store().await;
    seed_recall_shadow_rows(&store).await;

    let expected_all = recall_shadow_json(&store, RecallShadowListFilter::default()).await;
    let expected_routing = recall_shadow_json(
        &store,
        RecallShadowListFilter {
            routing: Some("search".to_owned()),
            ..Default::default()
        },
    )
    .await;
    let expected_scope = recall_shadow_json(
        &store,
        RecallShadowListFilter {
            scope_path: Some("global/project:helioy".to_owned()),
            ..Default::default()
        },
    )
    .await;
    let expected_top1 = recall_shadow_json(
        &store,
        RecallShadowListFilter {
            top1_changed: Some(true),
            ..Default::default()
        },
    )
    .await;

    let app = test_app(store);

    assert_eq!(
        expected_all,
        get_json(app.clone(), "/api/recall-shadow").await,
        "recall shadow list should match the store rows"
    );
    assert_eq!(
        expected_routing,
        get_json(app.clone(), "/api/recall-shadow?routing=search").await,
        "recall shadow routing filter should match the store rows"
    );
    assert_eq!(
        expected_scope,
        get_json(
            app.clone(),
            "/api/recall-shadow?scope_path=global/project:helioy"
        )
        .await,
        "recall shadow scope_path filter should match the store rows"
    );
    assert_eq!(
        expected_top1,
        get_json(app, "/api/recall-shadow?top1_changed=true").await,
        "recall shadow top1_changed filter should match the store rows"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_shadow_limit_is_clamped() {
    let (store, _dir) = test_store().await;
    for seed in 0..205 {
        store
            .log_recall_shadow(sample_record("search", None, seed % 2 == 0, seed))
            .await
            .unwrap();
    }

    let app = test_app(store);

    let min = get_json(app.clone(), "/api/recall-shadow?limit=0").await;
    assert_eq!(
        min["rows"].as_array().unwrap().len(),
        1,
        "limit=0 should clamp to the minimum page size"
    );

    let max = get_json(app, "/api/recall-shadow?limit=999").await;
    assert_eq!(
        max["rows"].as_array().unwrap().len(),
        200,
        "limit=999 should clamp to the maximum page size"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recall_shadow_summary_uses_all_matching_rows() {
    let (store, _dir) = test_store().await;
    for seed in 0..250u32 {
        let routing = if seed.is_multiple_of(2) {
            "search"
        } else {
            "scope_resolve"
        };
        store
            .log_recall_shadow(sample_record(routing, None, seed < 50, seed))
            .await
            .unwrap();
    }

    let app = test_app(store);
    let all = get_json(app.clone(), "/api/recall-shadow?limit=8").await;
    assert_eq!(all["rows"].as_array().unwrap().len(), 8);
    assert_eq!(all["summary"]["total"], json_number(250));
    assert_f64_eq(all["summary"]["divergence_rate"].as_f64().unwrap(), 0.2);

    let search = get_json(app, "/api/recall-shadow?routing=search&limit=8").await;
    assert_eq!(search["rows"].as_array().unwrap().len(), 8);
    assert_eq!(search["summary"]["total"], json_number(125));
    assert_f64_eq(search["summary"]["divergence_rate"].as_f64().unwrap(), 0.2);
}

async fn seed_recall_shadow_rows(store: &CmStore) {
    let rows = [
        sample_record("search", Some("global/project:helioy"), true, 1),
        sample_record("scope_resolve", Some("global/project:helioy"), false, 2),
        sample_record("tag_scope_walk", Some("global/project:other"), true, 3),
    ];

    for row in rows {
        store.log_recall_shadow(row).await.unwrap();
    }
}

async fn recall_shadow_json(store: &CmStore, filter: RecallShadowListFilter) -> Value {
    let response = RecallShadowResponse {
        summary: store.recall_shadow_summary(&filter).await.unwrap(),
        rows: store.list_recall_shadow(&filter).await.unwrap(),
    };
    serde_json::to_value(response).unwrap()
}

fn json_number(value: u64) -> Value {
    serde_json::json!(value)
}

fn assert_f64_eq(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

fn sample_record(
    routing: &str,
    scope_path: Option<&str>,
    top1_changed: bool,
    seed: u32,
) -> RecallShadowRecord {
    let old_id = uuid_from_seed(seed, 1);
    let new_id = uuid_from_seed(seed, 2);

    RecallShadowRecord {
        scope_path: scope_path.map(str::to_owned),
        query_hash: Some(format!("query-{seed}")),
        query_len: Some(seed + 10),
        routing: routing.to_owned(),
        tier: Some("fts".to_owned()),
        k: 3,
        candidate_count: seed + 3,
        top1_changed,
        topk_overlap: if top1_changed { 0.5 } else { 1.0 },
        footrule: f64::from(seed),
        mean_abs_position_delta: f64::from(seed) / 2.0,
        position_deltas: vec![RecallShadowPositionDelta {
            id: old_id,
            old_position: Some(0),
            new_position: Some(1),
            delta: 1,
        }],
        old_ids: vec![old_id],
        new_ids: vec![new_id],
        window_truncated: seed.is_multiple_of(2),
        ranking_version: "test".to_owned(),
        duration_ms: seed + 1,
    }
}

fn uuid_from_seed(seed: u32, offset: u32) -> Uuid {
    Uuid::from_u128(0x019dd3ad8ea27751ad871bd49e8bc000 + u128::from(seed * 16 + offset))
}
