//! Full text search tests.

mod common;

use cm_core::{EntryKind, NewScope};
use common::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c19_fts_search_finds_by_title() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Photosynthesis in plants",
                "Some body content",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("photosynthesis", None, 10).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.title, "Photosynthesis in plants");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c19_fts_search_finds_by_body() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Generic title",
                "The mitochondria is the powerhouse of the cell",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("mitochondria", None, 10).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.title, "Generic title");
}

/// FTS5 `rank` equals `bm25(entries_fts)`, a negative float where lower values
/// indicate higher relevance. Seed two entries with different keyword densities
/// for the same query term and assert the denser match ranks first.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fts_search_surfaces_bm25_score_and_preserves_ranking() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "sqlx migration guide",
                "sqlx migration sqlx migration sqlx",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Unrelated title",
                "A passing mention of sqlx somewhere in the text.",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("sqlx", None, 10).await.unwrap();
    assert_eq!(results.len(), 2);

    for scored in &results {
        assert!(scored.score.is_finite());
        assert!(scored.score < 0.0, "bm25 rank should be negative");
    }

    assert_eq!(results[0].entry.title, "sqlx migration guide");
    assert_eq!(results[1].entry.title, "Unrelated title");
    assert!(
        results[0].score < results[1].score,
        "dense match should have the more-negative bm25 score: {} vs {}",
        results[0].score,
        results[1].score,
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c20_fts_reflects_updated_content() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Interesting title",
                "Original content about elephants",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("Updated content about giraffes".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("giraffes", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);

    let results = store.search("elephants", None, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c21_superseded_entries_excluded_from_search() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Searchable entry",
                "Contains the word quantum",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store.search("quantum", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);

    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    let results = store.search("quantum", None, 10).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c25_updated_at_and_fts_both_fire_on_update() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Dual trigger test",
                "Original content keyword: albatross",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let updated = store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                body: Some("New content keyword: pelican".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert!(updated.updated_at > entry.updated_at);

    let found = store.search("pelican", None, 10).await.unwrap();
    assert_eq!(found.len(), 1);

    let not_found = store.search("albatross", None, 10).await.unwrap();
    assert_eq!(not_found.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_with_scope_filter() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let project_path = ScopePath::parse("global/project:scoped").unwrap();
    store
        .create_scope(
            NewScope {
                path: project_path.clone(),
                label: "Scoped".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global/project:other").unwrap(),
                label: "Other".to_owned(),
                meta: None,
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry(
                "global/project:scoped",
                EntryKind::Fact,
                "Scoped entry",
                "Contains the word butterfly",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global/project:other",
                EntryKind::Fact,
                "Other entry",
                "Also contains butterfly",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();
    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "Global entry",
                "Global butterfly too",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let results = store
        .search("butterfly", Some(&project_path), 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    let scopes: Vec<&str> = results
        .iter()
        .map(|s| s.entry.scope_path.as_str())
        .collect();
    assert!(scopes.contains(&"global/project:scoped"));
    assert!(scopes.contains(&"global"));
    assert!(!scopes.contains(&"global/project:other"));
}
