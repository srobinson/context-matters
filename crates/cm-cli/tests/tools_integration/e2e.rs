use cm_cli::mcp::tools;
use serde_json::json;

use crate::common::{count_row_lines, create_global, extract_stored_id, test_store};

#[tokio::test(flavor = "multi_thread")]
async fn e2e_store_recall_get_flow() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Store.
    let r = tools::cx_store(
        &store,
        &json!({
            "title": "E2E test entry",
            "body": "End-to-end test content for verification.",
            "kind": "lesson",
            "tags": ["testing", "e2e"]
        }),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    // Recall by query. Use a term without hyphens to avoid FTS5 parsing issues.
    let recall = tools::cx_recall(&store, &json!({"query": "verification"}))
        .await
        .unwrap()
        .text;
    assert!(recall.contains("routing: search"));
    assert!(count_row_lines(&recall) >= 1);

    // Get full content.
    let get = tools::cx_get(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(get.contains("found: 1"));
    assert!(get.contains("End-to-end"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_recall_id_round_trips_through_get() {
    // ALP-1767 regression: after ripping the short-id prefix input path
    // out of `cx_get`, only full hyphenated UUIDv7s parse. This test
    // proves the id that `cx_recall` surfaces in its structured payload
    // is directly usable against `cx_get` with no trimming, padding, or
    // hex manipulation in between.
    //
    // The YAML text channel does not render the uuid in row lines, so
    // the id must come from the JSON `structuredContent` payload that
    // the dual-channel response carries alongside the text.
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    tools::cx_store(
        &store,
        &json!({
            "title": "Round trip probe",
            "body": "Round trip probe body with unique marker widgetflange.",
            "kind": "reference"
        }),
    )
    .await
    .unwrap();

    let recall = tools::cx_recall(&store, &json!({"query": "widgetflange"}))
        .await
        .unwrap();
    let structured = recall
        .structured
        .as_ref()
        .expect("cx_recall must emit a structured payload");
    let entries = structured["entries"]
        .as_array()
        .expect("structured.entries must be an array");
    assert!(!entries.is_empty(), "recall must return at least one row");
    let recall_id = entries[0]["id"]
        .as_str()
        .expect("recall row must carry a string id")
        .to_owned();

    // The id returned by recall must parse as a full UUID and route
    // cleanly through cx_get without any pre-processing by the caller.
    assert!(
        uuid::Uuid::parse_str(&recall_id).is_ok(),
        "recall id `{recall_id}` must be a full hyphenated UUIDv7"
    );

    let get = tools::cx_get(&store, &json!({"ids": [&recall_id]}))
        .await
        .unwrap()
        .text;
    assert!(get.contains("found: 1"), "expected found: 1, got:\n{get}");
    assert!(!get.contains("missing: ["));
    assert!(get.contains("widgetflange"));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_store_update_verify() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Before update", "body": "Original.", "kind": "fact"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    tools::cx_update(
        &store,
        &json!({"id": &id, "title": "After update", "body": "Modified."}),
    )
    .await
    .unwrap();

    let get = tools::cx_get(&store, &json!({"ids": [&id]}))
        .await
        .unwrap()
        .text;
    assert!(get.contains("title: After update"));
    assert!(get.contains("Modified."));
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_store_forget_exclusion() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    let r = tools::cx_store(
        &store,
        &json!({"title": "Will vanish", "body": "Gone soon.", "kind": "observation"}),
    )
    .await
    .unwrap()
    .text;
    let id = extract_stored_id(&r);

    tools::cx_forget(&store, &json!({"ids": [&id]}))
        .await
        .unwrap();

    // Browse should not include it by default. Rows render the title
    // on the list line with no short id column after ALP-1767 phase 2,
    // so substring-check the unique title instead.
    let browse = tools::cx_browse(&store, &json!({})).await.unwrap().text;
    assert!(!browse.contains("Will vanish"));

    // Browse with include_superseded should include it.
    let browse2 = tools::cx_browse(&store, &json!({"include_superseded": true}))
        .await
        .unwrap()
        .text;
    assert!(browse2.contains("Will vanish"));
}
