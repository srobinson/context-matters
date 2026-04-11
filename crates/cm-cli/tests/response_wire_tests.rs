//! Unit tests for the MCP response-wire helpers introduced in ALP-1735.
//!
//! Covers:
//! * `yaml_response` — pass-through for pre-formatted YAML text.
//! * `cap_response` — 16 KB byte cap with newline-boundary clip and truncation
//!   advisory.
//! * `apply_cap_for_tool` — per-tool bypass (`cx_export` stays uncapped, every
//!   other `cx_*` tool is clipped).

use cm_cli::mcp::{MAX_MCP_RESPONSE_BYTES, apply_cap_for_tool, cap_response};
use cm_cli::yaml_response;

// ── yaml_response ────────────────────────────────────────────────

#[test]
fn yaml_response_passes_text_unchanged() {
    let input = "stored: abcd1234\nscope: global\nkind: fact\ncontent_hash: deadbeef\n".to_owned();
    let got = yaml_response(input.clone()).expect("yaml_response is infallible");
    // `yaml_response` now wraps the string in a text-only `ToolResult`
    // (ALP-1760 dual-channel envelope); the text channel must equal the
    // caller-provided YAML byte-for-byte and the structured channel must
    // be absent for write tools.
    assert_eq!(got.text, input);
    assert!(got.structured.is_none());
}

// ── cap_response ─────────────────────────────────────────────────

#[test]
fn cap_response_under_cap_unchanged() {
    let text = "line one\nline two\nline three\n".to_owned();
    let got = cap_response(text.clone(), MAX_MCP_RESPONSE_BYTES);
    assert_eq!(got, text);
}

#[test]
fn cap_response_over_cap_clips_at_newline_boundary() {
    // 100 rows of 200 'x' chars + '\n' = ~20,100 bytes, well over the 16 KB cap.
    let line = format!("{}\n", "x".repeat(200));
    let text: String = line.repeat(100);
    assert!(text.len() > MAX_MCP_RESPONSE_BYTES);

    let capped = cap_response(text, MAX_MCP_RESPONSE_BYTES);

    // The pre-advisory body must end at a newline so the clipped output stays
    // line-aligned.
    let advisory_start = capped
        .find("\n[Truncated")
        .expect("truncation advisory present");
    let body = &capped[..advisory_start];
    assert!(
        body.ends_with('\n'),
        "clipped body should end at a newline boundary"
    );
    // Body must fit inside the byte cap.
    assert!(body.len() <= MAX_MCP_RESPONSE_BYTES);
}

#[test]
fn cap_response_over_cap_no_newline_hard_clips() {
    // Pathological input: no newlines at all, twice the cap.
    let text: String = "x".repeat(MAX_MCP_RESPONSE_BYTES * 2);

    let capped = cap_response(text, MAX_MCP_RESPONSE_BYTES);

    // Advisory is appended, and the hard cut lands exactly at the byte cap
    // because no newline is available earlier in the buffer.
    let advisory_start = capped
        .find("\n[Truncated")
        .expect("truncation advisory present");
    assert_eq!(advisory_start, MAX_MCP_RESPONSE_BYTES);
}

#[test]
fn cap_response_appends_truncation_advisory() {
    let line = format!("{}\n", "y".repeat(200));
    let text: String = line.repeat(100);
    assert!(text.len() > MAX_MCP_RESPONSE_BYTES);

    let capped = cap_response(text, MAX_MCP_RESPONSE_BYTES);

    // Advisory mentions the 16 KB cap, cx_get, and clip reason so the LLM
    // recognises why output was shortened.
    assert!(capped.contains("[Truncated"));
    assert!(capped.contains("16 KB cap"));
    assert!(capped.contains("cx_get"));
}

// ── apply_cap_for_tool ───────────────────────────────────────────

#[test]
fn cap_response_export_bypass() {
    // Build a payload well above the cap so the difference between capped and
    // bypassed output is visible.
    let big: String = "x".repeat(MAX_MCP_RESPONSE_BYTES * 2);
    assert!(big.len() > MAX_MCP_RESPONSE_BYTES);

    // cx_export must bypass the cap: output is the exact input, byte-for-byte.
    let export_out = apply_cap_for_tool("cx_export", big.clone());
    assert_eq!(export_out.len(), big.len());
    assert_eq!(export_out, big);
    assert!(!export_out.contains("[Truncated"));

    // Every other tool is subject to the cap.
    for tool in ["cx_recall", "cx_browse", "cx_get", "cx_stats", "cx_store"] {
        let out = apply_cap_for_tool(tool, big.clone());
        assert!(out.len() < big.len(), "{tool} output should be capped");
        assert!(
            out.contains("[Truncated"),
            "{tool} output should carry the truncation advisory"
        );
    }
}
