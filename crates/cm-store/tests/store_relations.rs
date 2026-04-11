//! Relation operations: count_relations_for batch query.
//!
//! `store_query.rs` is pinned over the LOC limit, so the new
//! `count_relations_for` integration tests live here in their own file.

mod common;

use cm_core::{ContextStore, Entry, EntryKind, RelationKind};
use common::*;

async fn make_entry(store: &CmStore, scope: &str, title: &str) -> Entry {
    // Body must be unique per call: the store rejects duplicate content_hash.
    let body = format!("body for {title}");
    store
        .create_entry(
            new_entry(scope, EntryKind::Fact, title, &body),
            &test_ctx(),
        )
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn count_relations_for_empty_input_no_query() {
    // Empty input must short-circuit without touching the pool.
    // We close both pools first; if the implementation tries to query,
    // sqlx returns PoolClosed and the test fails.
    let (store, _dir) = test_store().await;
    store.read_pool().close().await;
    store.write_pool().close().await;

    let result = store
        .count_relations_for(&[])
        .await
        .expect("empty input must not touch the pool");
    assert!(result.is_empty(), "empty input must yield empty map");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn count_relations_for_returns_zero_for_orphans() {
    // Orphans (entries with no outgoing relations) are absent from the
    // returned map. Callers treat absence as zero via
    // `map.get(&id).copied().unwrap_or(0)`.
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let a = make_entry(&store, "global", "orphan-a").await;
    let b = make_entry(&store, "global", "orphan-b").await;

    let counts = store.count_relations_for(&[a.id, b.id]).await.unwrap();

    assert!(counts.is_empty(), "orphans should not appear in the map");
    assert_eq!(counts.get(&a.id).copied().unwrap_or(0), 0);
    assert_eq!(counts.get(&b.id).copied().unwrap_or(0), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn count_relations_for_counts_outgoing_edges() {
    // Only outgoing edges (where the id is `source_id`) are counted.
    // Incoming edges do not contribute. All RelationKind variants are
    // counted together (no per-kind breakdown).
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let source = make_entry(&store, "global", "source").await;
    let t1 = make_entry(&store, "global", "target-1").await;
    let t2 = make_entry(&store, "global", "target-2").await;
    let t3 = make_entry(&store, "global", "target-3").await;
    let other = make_entry(&store, "global", "other").await;

    // 3 outgoing edges from `source`, mixing relation kinds
    store
        .create_relation(source.id, t1.id, RelationKind::Elaborates, &test_ctx())
        .await
        .unwrap();
    store
        .create_relation(source.id, t2.id, RelationKind::RelatesTo, &test_ctx())
        .await
        .unwrap();
    store
        .create_relation(source.id, t3.id, RelationKind::DependsOn, &test_ctx())
        .await
        .unwrap();
    // 1 incoming edge to `source` — must NOT be counted for `source`.
    store
        .create_relation(other.id, source.id, RelationKind::RelatesTo, &test_ctx())
        .await
        .unwrap();

    let counts = store
        .count_relations_for(&[source.id, t1.id, other.id])
        .await
        .unwrap();

    assert_eq!(counts.get(&source.id).copied(), Some(3));
    // `other` has one outgoing edge (to source).
    assert_eq!(counts.get(&other.id).copied(), Some(1));
    // `t1` is a target of an edge but has no outgoing edges → omitted.
    assert!(!counts.contains_key(&t1.id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn count_relations_for_one_batch_query() {
    // Functional batch correctness: a single call with N ids and varied
    // outgoing-edge counts must return correct counts for every id.
    //
    // The implementation in `do_count_relations_for` uses a single
    // `sqlx::query(&sql).fetch_all(pool)` invocation with an `IN (?, ?, ...)`
    // clause. A regression that replaced this with a per-id loop would
    // change the SQL contract and is enforced by code review; this test
    // additionally guards against off-by-one and binding bugs in the
    // dynamic placeholder construction.
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // 6 source entries with edge counts: 0, 1, 2, 3, 4, 5
    let mut sources = Vec::with_capacity(6);
    for i in 0..6 {
        sources.push(make_entry(&store, "global", &format!("source-{i}")).await);
    }
    // Pool of 5 distinct targets
    let mut targets = Vec::with_capacity(5);
    for i in 0..5 {
        targets.push(make_entry(&store, "global", &format!("target-{i}")).await);
    }

    // Wire i edges from sources[i] to targets[0..i]
    for (i, source) in sources.iter().enumerate() {
        for target in targets.iter().take(i) {
            store
                .create_relation(source.id, target.id, RelationKind::RelatesTo, &test_ctx())
                .await
                .unwrap();
        }
    }

    // One batched call across all 6 source ids
    let ids: Vec<_> = sources.iter().map(|e| e.id).collect();
    let counts = store.count_relations_for(&ids).await.unwrap();

    // sources[0] has 0 outgoing edges → omitted from the map
    assert!(!counts.contains_key(&sources[0].id));
    for (i, source) in sources.iter().enumerate().skip(1) {
        assert_eq!(
            counts.get(&source.id).copied(),
            Some(i as u32),
            "expected source-{i} to have {i} outgoing edges",
        );
    }
    // Total entries in the map equals the number of non-orphan sources.
    assert_eq!(counts.len(), 5);
}
