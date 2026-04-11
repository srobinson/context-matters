//! Capability-level tests for the FTS5 recall fallback cascade.
//!
//! Exercises `cm_capabilities::recall::recall()` directly against a real
//! SQLite store, asserting which tier of the `SearchTier` cascade returns
//! rows for each query shape. A sibling file `recall_tests.rs` covers the
//! broader routing / filtering / budget surface; these tests focus solely
//! on the cascade and on the `tier` field threaded through `RecallResult`.
//!
//! Helpers here are deliberately a minimal subset of `recall_tests.rs`'s
//! fixtures. Sharing a common module is out of scope for ALP-1747; the
//! three existing `tests/*.rs` files in `cm-capabilities` each inline
//! their own `test_store` helper, and this file follows the same
//! convention so the new tier coverage can land without touching any
//! grandfathered test file.

use cm_capabilities::recall::{RecallRequest, RecallRouting, SearchTier, recall};
use cm_core::{
    ContextStore, EntryKind, MutationSource, NewEntry, NewScope, ScopePath, WriteContext,
};
use cm_store::{CmStore, schema};

// ── Helpers ──────────────────────────────────────────────────────

async fn test_store() -> (CmStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let (write_pool, read_pool) = schema::create_pools(&db_path).await.unwrap();
    schema::run_migrations(&write_pool).await.unwrap();
    let store = CmStore::new(write_pool, read_pool);
    (store, dir)
}

fn wctx() -> WriteContext {
    WriteContext::new(MutationSource::Mcp)
}

async fn create_global(store: &CmStore) {
    store
        .create_scope(
            NewScope {
                path: ScopePath::parse("global").unwrap(),
                label: "Global".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

async fn seed_entry(store: &CmStore, title: &str, body: &str, kind: EntryKind) {
    store
        .create_entry(
            NewEntry {
                scope_path: ScopePath::global(),
                kind,
                title: title.to_owned(),
                body: body.to_owned(),
                created_by: "test:seed".to_owned(),
                meta: None,
            },
            &wctx(),
        )
        .await
        .unwrap();
}

// ── Cascade: Exact ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tier_exact_fires_when_all_tokens_match() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Rust ownership guide",
        "Ownership and borrowing are the foundations of safe Rust.",
        EntryKind::Reference,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("ownership borrowing".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::Exact));
    assert!(!result.entries.is_empty());
    assert_eq!(result.entries[0].entry.title, "Rust ownership guide");
}

// ── Cascade: Prefix ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tier_prefix_fires_when_exact_is_zero() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    // Body tokens are "ownership" and "borrowing". Query uses prefixes
    // "owner" and "borrow", neither of which is a whole-token match, so
    // the Exact tier returns zero. Prefix tier (`owner* borrow*`) should
    // hit both tokens.
    seed_entry(
        &store,
        "Memory model notes",
        "ownership borrowing lifetimes.",
        EntryKind::Reference,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("owner borrow".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::Prefix));
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.title, "Memory model notes");
}

// ── Cascade: SplitOr ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tier_split_or_fires_when_prefix_is_zero() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    // Neither entry contains both "alpha" and "beta". Exact tier
    // (`alpha AND beta`) returns zero. Prefix tier (`alpha* beta*`) also
    // returns zero for the same reason. SplitOr (`alpha OR beta`) should
    // surface both rows.
    seed_entry(
        &store,
        "Alpha note",
        "This is about alpha only.",
        EntryKind::Fact,
    )
    .await;
    seed_entry(
        &store,
        "Beta note",
        "This is about beta only.",
        EntryKind::Fact,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("alpha beta".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::SplitOr));
    assert_eq!(result.entries.len(), 2);
}

// ── Cascade: None ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tier_none_when_all_tiers_exhausted() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Ownership guide",
        "ownership borrowing lifetimes.",
        EntryKind::Reference,
    )
    .await;

    // Gibberish tokens match nothing in Exact, Prefix, or SplitOr.
    let result = recall(
        &store,
        RecallRequest {
            query: Some("zzgibberishxyz qqwombatzz".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::None));
    assert!(result.entries.is_empty());
}

// ── Cascade: advances past Prefix on reserved-word query (ALP-1765) ─

#[tokio::test(flavor = "multi_thread")]
async fn cascade_advances_past_prefix_on_uppercase_not() {
    // Regression for the field-report bug where the Prefix tier crashed
    // FTS5 with `syntax error near "*"` whenever a query contained an
    // uppercase reserved word. Before the fix this `.unwrap()` panicked
    // because `try_search_tier` propagated the database error instead of
    // letting the cascade advance.
    //
    // Seeded entry contains only "operators" so:
    //
    //   * Exact tier: `FtsQuery::new` preserves `NOT`, FTS5 parses as
    //     `FTS5 AND sanitization AND hyphens AND (NOT operators)`. The
    //     entry has none of FTS5/sanitization/hyphens, so 0 rows.
    //   * Prefix tier (post-fix): `NOT` is stripped, query is
    //     `FTS5* sanitization* hyphens* operators*` (implicit AND). Entry
    //     matches only `operators*`, so 0 rows. Critically: no crash.
    //   * SplitOr tier: query is `FTS5 OR sanitization OR hyphens OR
    //     operators`. Entry hits on `operators`. 1 row.
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Operator notes",
        "Tokenizer operators only.",
        EntryKind::Fact,
    )
    .await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("FTS5 sanitization hyphens NOT operators".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .expect("cascade must not crash on uppercase NOT");

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::SplitOr));
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.title, "Operator notes");
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_advances_past_prefix_on_uppercase_and() {
    // Same shape as the `NOT` case but for `AND`. The Exact tier preserves
    // `AND` as the explicit boolean operator, so for two tokens the parse
    // is `alpha AND zzgibberishxyz` which finds nothing. The Prefix tier
    // strips `AND`, leaving `alpha* zzgibberishxyz*` (implicit AND), still
    // 0. SplitOr broadens to `alpha OR zzgibberishxyz`, hits on `alpha`.
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Alpha note", "alpha only.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("alpha AND zzgibberishxyz".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .expect("cascade must not crash on uppercase AND");

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::SplitOr));
    assert_eq!(result.entries.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_advances_past_prefix_on_uppercase_or() {
    // Mirror of the `NOT` and `AND` cases for `OR`. Before the fix, the
    // Prefix tier crashed on the bare `OR*` token; after the fix `OR` is
    // stripped so the tier is a clean no-op and the cascade advances to
    // SplitOr, which rescues.
    //
    // Seeded entry contains only "widget" so:
    //
    //   * Exact tier: `FtsQuery::new` preserves `OR` as a boolean. FTS5
    //     parses `alpha OR widget beta` as `alpha OR (widget AND beta)`
    //     because AND binds tighter than OR. The row has `widget` but no
    //     `beta`, so the AND side is 0. No `alpha` either. 0 rows.
    //   * Prefix tier (post-fix): `OR` is stripped, query is
    //     `alpha* widget* beta*` (implicit AND). Row matches only
    //     `widget*`, so 0 rows. Critically: no crash.
    //   * SplitOr tier: query is `alpha OR widget OR beta`. Row matches
    //     on `widget`. 1 row.
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(&store, "Widget note", "widget only.", EntryKind::Fact).await;

    let result = recall(
        &store,
        RecallRequest {
            query: Some("alpha OR widget beta".to_owned()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .expect("cascade must not crash on uppercase OR");

    assert_eq!(result.routing, RecallRouting::Search);
    assert_eq!(result.tier, Some(SearchTier::SplitOr));
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry.title, "Widget note");
}

// ── Non-search routings leave tier as None ───────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn non_search_routing_leaves_tier_none() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Scope resolve target",
        "anything goes",
        EntryKind::Fact,
    )
    .await;

    // No `query` → routing is `ScopeResolve`, cascade is not entered,
    // and `tier` must remain `None` (distinct from `Some(SearchTier::None)`).
    let result = recall(
        &store,
        RecallRequest {
            query: None,
            scope: Some(ScopePath::global()),
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.routing, RecallRouting::ScopeResolve);
    assert_eq!(result.tier, None);
}
