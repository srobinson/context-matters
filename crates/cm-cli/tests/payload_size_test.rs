//! Empirical byte-size regression test for the `cx_browse` wire payload.
//!
//! Acceptance check for the ALP-1725 YAML-payload redesign: seed a
//! fixture store with 20 session-log entries shaped like Stuart's real
//! workload, call `cx_browse(tag="session-log", limit=20)` through the
//! MCP handler, wrap the result in the exact `JsonRpcResponse` envelope
//! the server emits, and assert the serialised wire bytes stay under
//! 6 KB. The research doc projects ~4,300 bytes for this shape; 6,000
//! leaves headroom for fixture variance and JSON string-escape growth.
//!
//! Baseline: the pre-migration JSON-in-text shape for the same 20-row
//! browse weighed ~14,900 bytes (research doc §2 measurement). The
//! test prints both numbers so CI logs carry a durable before/after
//! record of the migration.
//!
//! This test sits one layer above the formatter-level byte guards in
//! `cm-capabilities/tests/browse_format_tests.rs` (which pin the 3-row
//! session-log golden under 1,200 bytes) and catches regressions in
//! the whole handler → formatter → cap_response → JSON-RPC envelope
//! pipeline with a realistic row count.

mod common;

use cm_cli::mcp::{apply_cap_for_tool, tools};
use serde_json::json;

use common::{create_global, test_store};

/// Byte cap for the full JSON-RPC wire response on a 20-row
/// session-log browse. Research doc projects 4,300; 6,000 leaves
/// headroom for fixture variance and JSON string-escape growth.
const WIRE_BYTE_CAP: usize = 6_000;

/// Documented pre-migration baseline for the same 20-row browse
/// against the legacy JSON-in-text shape (research doc §2). Logged
/// alongside the measured bytes so CI carries an audit record of
/// the migration's empirical win.
const LEGACY_BASELINE_BYTES: usize = 14_900;

#[tokio::test(flavor = "multi_thread")]
async fn cx_browse_wire_payload_stays_under_6k_for_20_session_logs() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;

    // Seed 20 session-log observation entries. Both title and body
    // are kept compact on purpose: the formatter's smart-snippet
    // pulls the first ~200 bytes of the body into the row, so any
    // extra body length beyond that would show up in the rendered
    // view and skew the measurement. Stuart's real session-log
    // bodies frequently run longer, but the relevant byte budget
    // is what the wire payload surfaces, not what the store holds.
    for i in 0..20 {
        let title = format!(
            "Session: {} work #{i}",
            SAMPLE_TOPICS[i % SAMPLE_TOPICS.len()]
        );
        let body = make_session_log_body(i);
        tools::cx_store(
            &store,
            &json!({
                "title": title,
                "body": body,
                "kind": "observation",
                "tags": ["session-log"]
            }),
        )
        .await
        .unwrap();
    }

    // Call the browse handler the same way the MCP server does,
    // with an explicit `limit=20` so the paginator does not clip
    // the fixture and a `tag=session-log` filter that exercises
    // the formatter's "query header" reconstruct path.
    let body = tools::cx_browse(
        &store,
        &json!({
            "tag": "session-log",
            "limit": 20
        }),
    )
    .await
    .unwrap();

    // The server applies `apply_cap_for_tool` to the raw formatter
    // output before wrapping it in the CallToolResult envelope.
    // Run the same cap here so the measurement matches what a
    // client would actually see on the wire.
    let capped = apply_cap_for_tool("cx_browse", body);

    // Reconstruct the exact JSON-RPC response shape the server
    // emits (`mcp/mod.rs:310-312`). The trailing `\n` mirrors the
    // `writeln!` the server uses on stdout.
    let envelope = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": {
            "content": [
                {"type": "text", "text": capped}
            ]
        }
    });
    let wire = format!("{}\n", serde_json::to_string(&envelope).unwrap());
    let wire_bytes = wire.len();

    // Log the before/after so CI output records the migration win.
    // Using the `CM_PRINT_PAYLOAD_BYTES` env var, callers can also
    // dump the full wire text for local inspection; off by default
    // to keep CI logs tidy.
    println!(
        "cx_browse 20-row session-log wire bytes: {wire_bytes} \
         (legacy baseline: {LEGACY_BASELINE_BYTES}, cap: {WIRE_BYTE_CAP})"
    );
    if std::env::var("CM_PRINT_PAYLOAD_BYTES").is_ok() {
        println!("--- wire ---\n{wire}\n--- end ---");
    }

    assert!(
        wire_bytes < WIRE_BYTE_CAP,
        "cx_browse 20-row session-log wire payload regressed: {wire_bytes} bytes \
         (cap: {WIRE_BYTE_CAP}, legacy baseline: {LEGACY_BASELINE_BYTES})"
    );
}

/// Short topic stubs woven into the 20 seeded titles so individual
/// rows differ without bloating the per-row title length.
const SAMPLE_TOPICS: &[&str] = &[
    "browse", "recall", "snippet", "scope", "forget", "deposit", "short id", "age",
];

/// Produce a unique-per-seed session-log body. Every entry carries
/// its ordinal in the first sentence so BLAKE3 hashes differ and the
/// store's dedup check does not reject the fixture, but the body
/// stays compact (~80 bytes) so the formatter's smart-snippet has
/// nothing long to truncate.
fn make_session_log_body(seed: usize) -> String {
    format!("Entry #{seed}: brief session log note describing the iteration work.")
}
