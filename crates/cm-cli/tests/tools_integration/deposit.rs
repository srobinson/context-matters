use cm_cli::mcp::tools;
use cm_core::ContextStore;
use serde_json::json;

use crate::common::{create_global, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn deposit_creates_exchange_entries() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_deposit(
        &store,
        &json!({
            "exchanges": [
                {"user": "How do I parse JSON?", "assistant": "Use serde_json::from_str."},
                {"user": "What about errors?", "assistant": "Use the ? operator with Result."}
            ]
        }),
    )
    .await
    .unwrap()
    .text;
    // `format_deposit_ack` pluralises `exchange` and, without a summary,
    // renders an inline `entry_ids: [id1, id2]` list of 8 char shorts.
    assert!(result.contains("deposited: 2 exchanges"));
    assert!(result.contains("entry_ids: ["));
    // Two ids in the list means one comma separator; zero summary means
    // no `summary:` line at all.
    let id_line = result
        .lines()
        .find(|l| l.starts_with("entry_ids: ["))
        .expect("entry_ids line present");
    assert_eq!(id_line.matches(',').count(), 1);
    assert!(!result.contains("summary:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_with_summary_creates_relations() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let result = tools::cx_deposit(
        &store,
        &json!({
            "exchanges": [
                {"user": "What is Rust?", "assistant": "A systems programming language."}
            ],
            "summary": "Discussed Rust programming language basics."
        }),
    )
    .await
    .unwrap()
    .text;
    // Single exchange renders singular `exchange` with no trailing `s`. With a
    // summary present, `format_deposit_ack` suppresses the per entry
    // `entry_ids` list and surfaces the summary's full uuid instead.
    assert!(result.contains("deposited: 1 exchange\n"));
    assert!(result.contains("summary: "));
    assert!(!result.contains("entry_ids: ["));
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_rejects_empty_exchanges() {
    let (store, _dir) = test_store().await;
    let result = tools::cx_deposit(&store, &json!({"exchanges": []})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_rejects_removed_scope_inputs_before_writing() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    for (args, expected) in [
        (
            json!({
                "exchanges": [{"user": "u", "assistant": "a"}],
                "scope_path": "global"
            }),
            "use 'scope' instead of 'scope_path'",
        ),
        (
            json!({
                "exchanges": [{"user": "u", "assistant": "a"}],
                "scope": "auto"
            }),
            "instead of scope='auto'",
        ),
        (
            json!({
                "exchanges": [{"user": "u", "assistant": "a"}],
                "scope_mode": "resolved"
            }),
            "use 'scope' instead of 'scope_mode'",
        ),
    ] {
        let err = tools::cx_deposit(&store, &args).await.unwrap_err();
        assert!(err.contains(expected), "expected {expected:?}, got {err:?}");
    }

    assert_eq!(store.export(None).await.unwrap().len(), 0);
}
