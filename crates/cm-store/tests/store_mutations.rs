//! Mutation history logging integration tests.

mod common;

use chrono::Utc;
use cm_core::{ContextStore, EntryKind, MutationAction, MutationSource};
use common::*;

// ── create_entry writes mutation ────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_entry_writes_mutation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Test title", "Test body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let mutations = store.get_mutations(entry.id, 50, 0).await.unwrap();
    assert_eq!(mutations.len(), 1);

    let m = &mutations[0];
    assert_eq!(m.entry_id, entry.id);
    assert_eq!(m.action, MutationAction::Create);
    assert_eq!(m.source, MutationSource::Mcp);
    assert!(m.before_snapshot.is_none());

    let after = m.after_snapshot.as_ref().unwrap();
    assert_eq!(after["title"], "Test title");
    assert_eq!(after["body"], "Test body");
    assert_eq!(after["id"], entry.id.to_string());
}

// ── update_entry writes mutation ────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_entry_writes_mutation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Original", "Original body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .update_entry(
            entry.id,
            cm_core::UpdateEntry {
                title: Some("Updated".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let mutations = store.get_mutations(entry.id, 50, 0).await.unwrap();
    // get_mutations returns DESC order, so Update is first
    assert_eq!(mutations.len(), 2);
    assert_eq!(mutations[0].action, MutationAction::Update);
    assert_eq!(mutations[1].action, MutationAction::Create);

    let update_m = &mutations[0];
    let before = update_m.before_snapshot.as_ref().unwrap();
    let after = update_m.after_snapshot.as_ref().unwrap();
    assert_eq!(before["title"], "Original");
    assert_eq!(after["title"], "Updated");
}

// ── forget_entry writes mutation ────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_entry_writes_mutation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Will forget", "body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    let mutations = store.get_mutations(entry.id, 50, 0).await.unwrap();
    assert_eq!(mutations.len(), 2);
    assert_eq!(mutations[0].action, MutationAction::Forget);
    assert_eq!(mutations[1].action, MutationAction::Create);

    let forget_m = &mutations[0];
    let before = forget_m.before_snapshot.as_ref().unwrap();
    let after = forget_m.after_snapshot.as_ref().unwrap();

    // Before: no superseded_by
    assert!(before["superseded_by"].is_null());
    // After: superseded_by = self
    assert_eq!(after["superseded_by"], entry.id.to_string());
}

// ── supersede_entry writes two mutations ────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supersede_entry_writes_two_mutations() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let old = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Old", "old body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let new = store
        .supersede_entry(
            old.id,
            new_entry("global", EntryKind::Fact, "New", "new body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    // Old entry: Create + Supersede
    let old_mutations = store.get_mutations(old.id, 50, 0).await.unwrap();
    assert_eq!(old_mutations.len(), 2);
    assert_eq!(old_mutations[0].action, MutationAction::Supersede);
    assert_eq!(old_mutations[1].action, MutationAction::Create);

    let supersede_m = &old_mutations[0];
    let before = supersede_m.before_snapshot.as_ref().unwrap();
    let after = supersede_m.after_snapshot.as_ref().unwrap();
    assert!(before["superseded_by"].is_null());
    assert_eq!(after["superseded_by"], new.id.to_string());

    // New entry: Create only
    let new_mutations = store.get_mutations(new.id, 50, 0).await.unwrap();
    assert_eq!(new_mutations.len(), 1);
    assert_eq!(new_mutations[0].action, MutationAction::Create);
    assert!(new_mutations[0].before_snapshot.is_none());

    let new_after = new_mutations[0].after_snapshot.as_ref().unwrap();
    assert_eq!(new_after["title"], "New");
}

// ── No-op cases ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_already_superseded_no_mutation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Will forget twice", "body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store.forget_entry(entry.id, &test_ctx()).await.unwrap();
    // Second forget is a no-op
    store.forget_entry(entry.id, &test_ctx()).await.unwrap();

    let mutations = store.get_mutations(entry.id, 50, 0).await.unwrap();
    // Only Create + Forget, no second Forget
    assert_eq!(mutations.len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn noop_update_no_mutation() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Static", "body"),
            &test_ctx(),
        )
        .await
        .unwrap();

    // Update with no changes
    store
        .update_entry(entry.id, cm_core::UpdateEntry::default(), &test_ctx())
        .await
        .unwrap();

    let mutations = store.get_mutations(entry.id, 50, 0).await.unwrap();
    // Only the initial Create, no Update mutation
    assert_eq!(mutations.len(), 1);
    assert_eq!(mutations[0].action, MutationAction::Create);
}

// ── list_mutations filters ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_mutations_filters() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let before_create = Utc::now();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let e1 = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Entry 1", "body-1"),
            &test_ctx(),
        )
        .await
        .unwrap();

    let e2 = store
        .create_entry(
            new_entry("global", EntryKind::Decision, "Entry 2", "body-2"),
            &test_ctx(),
        )
        .await
        .unwrap();

    store
        .update_entry(
            e1.id,
            cm_core::UpdateEntry {
                title: Some("Entry 1 updated".to_owned()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let after_all = Utc::now();

    // Filter by action
    let creates = store
        .list_mutations(None, Some(MutationAction::Create), None, None, None, 100)
        .await
        .unwrap();
    assert_eq!(creates.len(), 2); // e1 Create + e2 Create

    let updates = store
        .list_mutations(None, Some(MutationAction::Update), None, None, None, 100)
        .await
        .unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].entry_id, e1.id);

    // Filter by entry_id
    let e2_mutations = store
        .list_mutations(Some(e2.id), None, None, None, None, 100)
        .await
        .unwrap();
    assert_eq!(e2_mutations.len(), 1);
    assert_eq!(e2_mutations[0].action, MutationAction::Create);

    // Filter by source
    let mcp_mutations = store
        .list_mutations(None, None, Some(MutationSource::Mcp), None, None, 100)
        .await
        .unwrap();
    assert_eq!(mcp_mutations.len(), 3); // 2 Creates + 1 Update

    let cli_mutations = store
        .list_mutations(None, None, Some(MutationSource::Cli), None, None, 100)
        .await
        .unwrap();
    assert_eq!(cli_mutations.len(), 0);

    // Filter by timestamp range
    let since_mutations = store
        .list_mutations(None, None, None, Some(before_create), None, 100)
        .await
        .unwrap();
    assert_eq!(since_mutations.len(), 3);

    let until_mutations = store
        .list_mutations(None, None, None, None, Some(after_all), 100)
        .await
        .unwrap();
    assert_eq!(until_mutations.len(), 3);

    // Limit
    let limited = store
        .list_mutations(None, None, None, None, None, 2)
        .await
        .unwrap();
    assert_eq!(limited.len(), 2);
}

// ── get_mutations pagination ────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_mutations_pagination() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let entry = store
        .create_entry(
            new_entry("global", EntryKind::Fact, "Paginated", "body-0"),
            &test_ctx(),
        )
        .await
        .unwrap();

    // Perform 4 updates to get 5 total mutations (1 Create + 4 Update)
    for i in 1..=4 {
        store
            .update_entry(
                entry.id,
                cm_core::UpdateEntry {
                    body: Some(format!("body-{i}")),
                    ..Default::default()
                },
                &test_ctx(),
            )
            .await
            .unwrap();
    }

    let all = store.get_mutations(entry.id, 50, 0).await.unwrap();
    assert_eq!(all.len(), 5);

    // Page 1: first 2
    let page1 = store.get_mutations(entry.id, 2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);

    // Page 2: next 2
    let page2 = store.get_mutations(entry.id, 2, 2).await.unwrap();
    assert_eq!(page2.len(), 2);

    // Page 3: last 1
    let page3 = store.get_mutations(entry.id, 2, 4).await.unwrap();
    assert_eq!(page3.len(), 1);

    // No overlap
    let all_ids: Vec<_> = page1
        .iter()
        .chain(page2.iter())
        .chain(page3.iter())
        .map(|m| m.id)
        .collect();
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(all_ids.len(), unique.len());

    // DESC ordering: most recent first
    assert_eq!(page1[0].action, MutationAction::Update);
    assert_eq!(page3[0].action, MutationAction::Create);
}

// ── Enum roundtrip tests ────────────────────────────────────────

#[test]
fn mutation_source_roundtrip() {
    let variants = [
        MutationSource::Mcp,
        MutationSource::Cli,
        MutationSource::Web,
        MutationSource::Helix,
    ];
    for v in variants {
        let s = v.as_str();
        let parsed: MutationSource = s.parse().unwrap();
        assert_eq!(parsed, v, "roundtrip failed for {s}");
        assert_eq!(v.to_string(), s);
    }
}

#[test]
fn mutation_action_roundtrip() {
    let variants = [
        MutationAction::Create,
        MutationAction::Update,
        MutationAction::Forget,
        MutationAction::Supersede,
    ];
    for v in variants {
        let s = v.as_str();
        let parsed: MutationAction = s.parse().unwrap();
        assert_eq!(parsed, v, "roundtrip failed for {s}");
        assert_eq!(v.to_string(), s);
    }
}
