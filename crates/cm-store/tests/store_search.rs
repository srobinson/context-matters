//! Full text search tests.

mod common;

use cm_core::{
    AncestorWalkRequest, ContentSearchRequest, EntryKind, EntryMeta, NewEntry, NewScope,
    ScopeFilter, ScoredEntry,
};
use common::*;

async fn search_ancestor_walk(store: &CmStore, query: &str, scope: ScopePath) -> Vec<ScoredEntry> {
    store
        .do_search_ancestor_walk(AncestorWalkRequest {
            query: query.to_owned(),
            scope,
            limit: 10,
        })
        .await
        .unwrap()
}

fn tagged_entry(scope: &str, kind: EntryKind, title: &str, body: &str, tags: &[&str]) -> NewEntry {
    let mut entry = new_entry(scope, kind, title, body);
    entry.meta = Some(EntryMeta {
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        ..Default::default()
    });
    entry
}

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

    let results = search_ancestor_walk(&store, "photosynthesis", ScopePath::global()).await;

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

    let results = search_ancestor_walk(&store, "mitochondria", ScopePath::global()).await;

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

    let results = search_ancestor_walk(&store, "sqlx", ScopePath::global()).await;
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

    let results = search_ancestor_walk(&store, "giraffes", ScopePath::global()).await;
    assert_eq!(results.len(), 1);

    let results = search_ancestor_walk(&store, "elephants", ScopePath::global()).await;
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

    let results = search_ancestor_walk(&store, "quantum", ScopePath::global()).await;
    assert_eq!(results.len(), 1);

    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    let results = search_ancestor_walk(&store, "quantum", ScopePath::global()).await;
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

    let found = search_ancestor_walk(&store, "pelican", ScopePath::global()).await;
    assert_eq!(found.len(), 1);

    let not_found = search_ancestor_walk(&store, "albatross", ScopePath::global()).await;
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

    let results = search_ancestor_walk(&store, "butterfly", project_path).await;
    assert_eq!(results.len(), 2);

    let scopes: Vec<&str> = results
        .iter()
        .map(|s| s.entry.scope_path.as_str())
        .collect();
    assert!(scopes.contains(&"global/project:scoped"));
    assert!(scopes.contains(&"global"));
    assert!(!scopes.contains(&"global/project:other"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn content_search_applies_kind_filter_before_limit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "dense sqlite note",
                "sqlite sqlite sqlite sqlite sqlite",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Decision,
                "decision sqlite note",
                "sqlite once",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let page = store
        .do_content_search(ContentSearchRequest {
            query: "sqlite".to_owned(),
            scope: ScopeFilter::All,
            kinds: Some(vec![EntryKind::Decision]),
            tags: None,
            limit: 1,
            cursor: None,
        })
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].entry.kind, EntryKind::Decision);
    assert_eq!(page.items[0].entry.title, "decision sqlite note");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn content_search_filters_each_scope_variant() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for scope in [
        "global/project:alpha",
        "global/project:alpha/repo:one",
        "global/project:beta",
    ] {
        store
            .create_scope(
                NewScope {
                    path: ScopePath::parse(scope).unwrap(),
                    label: scope.rsplit('/').next().unwrap_or(scope).to_owned(),
                    meta: None,
                },
                &test_ctx(),
            )
            .await
            .unwrap();
    }

    for (scope, title) in [
        ("global", "global sqlite"),
        ("global/project:alpha", "alpha sqlite"),
        ("global/project:alpha/repo:one", "alpha repo sqlite"),
        ("global/project:beta", "beta sqlite"),
    ] {
        store
            .create_entry(
                new_entry(scope, EntryKind::Fact, title, "shared sqlite content"),
                &test_ctx(),
            )
            .await
            .unwrap();
    }

    let exact = sorted_content_search_titles(
        &store,
        ScopeFilter::Exact(ScopePath::parse("global/project:alpha").unwrap()),
    )
    .await;
    assert_eq!(exact, vec!["alpha sqlite"]);

    let subtree = sorted_content_search_titles(
        &store,
        ScopeFilter::Subtree(ScopePath::parse("global/project:alpha").unwrap()),
    )
    .await;
    assert_eq!(subtree, vec!["alpha repo sqlite", "alpha sqlite"]);

    let set = sorted_content_search_titles(
        &store,
        ScopeFilter::Set(vec![
            ScopePath::parse("global/project:alpha/repo:one").unwrap(),
            ScopePath::parse("global/project:beta").unwrap(),
        ]),
    )
    .await;
    assert_eq!(set, vec!["alpha repo sqlite", "beta sqlite"]);

    let all = sorted_content_search_titles(&store, ScopeFilter::All).await;
    assert_eq!(
        all,
        vec![
            "alpha repo sqlite",
            "alpha sqlite",
            "beta sqlite",
            "global sqlite"
        ]
    );
}

async fn sorted_content_search_titles(store: &CmStore, scope: ScopeFilter) -> Vec<String> {
    let mut titles: Vec<String> = store
        .do_content_search(ContentSearchRequest {
            query: "sqlite".to_owned(),
            scope,
            kinds: None,
            tags: None,
            limit: 10,
            cursor: None,
        })
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.entry.title)
        .collect();
    titles.sort();
    titles
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn content_search_applies_tag_filter_before_limit() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    store
        .create_entry(
            new_entry(
                "global",
                EntryKind::Fact,
                "dense untagged sqlite note",
                "sqlite sqlite sqlite sqlite sqlite",
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .create_entry(
            tagged_entry(
                "global",
                EntryKind::Fact,
                "tagged sqlite note",
                "sqlite once",
                &["target"],
            ),
            &test_ctx(),
        )
        .await
        .unwrap();

    let page = store
        .do_content_search(ContentSearchRequest {
            query: "sqlite".to_owned(),
            scope: ScopeFilter::All,
            kinds: None,
            tags: Some(vec!["target".to_owned()]),
            limit: 1,
            cursor: None,
        })
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].entry.title, "tagged sqlite note");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn content_search_cursor_round_trip_remains_stable_with_filters() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for title in ["alpha sqlite", "beta sqlite", "gamma sqlite"] {
        store
            .create_entry(
                tagged_entry(
                    "global",
                    EntryKind::Fact,
                    title,
                    &format!("{title} body"),
                    &["pageable"],
                ),
                &test_ctx(),
            )
            .await
            .unwrap();
    }

    let first = store
        .do_content_search(ContentSearchRequest {
            query: "sqlite".to_owned(),
            scope: ScopeFilter::All,
            kinds: Some(vec![EntryKind::Fact]),
            tags: Some(vec!["pageable".to_owned()]),
            limit: 2,
            cursor: None,
        })
        .await
        .unwrap();

    let second = store
        .do_content_search(ContentSearchRequest {
            query: "sqlite".to_owned(),
            scope: ScopeFilter::All,
            kinds: Some(vec![EntryKind::Fact]),
            tags: Some(vec!["pageable".to_owned()]),
            limit: 2,
            cursor: first.next_cursor.clone(),
        })
        .await
        .unwrap();

    assert_eq!(first.items.len(), 2);
    assert_eq!(second.items.len(), 1);
    assert!(first.next_cursor.is_some());
    assert!(second.next_cursor.is_none());
    assert_ne!(first.items[0].entry.id, second.items[0].entry.id);
    assert_ne!(first.items[1].entry.id, second.items[0].entry.id);
}
