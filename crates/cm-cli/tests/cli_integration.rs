//! End-to-end CLI integration tests against an isolated SQLite store.
//!
//! Each test creates its own `tempfile::TempDir`, points the binary at
//! it via `CM_DATA_DIR`, and exercises the full
//! `cm` CLI → `cm-capabilities` → `cm-store` pipeline by spawning the
//! compiled `cm` binary. Tests share no state and clean up automatically
//! when the `TempDir` drops at the end of the function.
//!
//! No CLI snapshot tests live here. Projection output formatting is
//! covered by snapshot tests at the `cm-capabilities` layer, where MCP
//! and CLI share the same `format_*_view` functions. These integration
//! tests assert pipeline wiring, not output bytes.

use std::path::Path;

use assert_cmd::Command;
use cm_capabilities::recall::RECALL_SCOPE_DEFAULT_ADVISORY;
use cm_capabilities::validation::{parse_kind, parse_tag_sort};
use predicates::str::contains;
use serde_json::Value;
use tempfile::tempdir;

/// Spawn `cm` with `CM_DATA_DIR` pointed at a per-test tempdir. The
/// store auto-creates on first use (the binary's `open_store` runs
/// `create_dir_all` and applies migrations), so no `cm init` is needed
/// before exercising other subcommands.
fn cm_with_data_dir(data_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cm").unwrap();
    cmd.env("CM_DATA_DIR", data_dir);
    cmd
}

/// Deposit a single exchange into a fresh store and return the
/// resulting entry ID. The recall human view does not surface entry
/// IDs, so the `-j` channel is the only path to a parseable identifier
/// without YAML scraping.
fn deposit_one_and_extract_id(data_dir: &Path, title: &str, body: &str) -> String {
    let exchanges = serde_json::json!([{
        "user": body,
        "assistant": "auto-reply",
        "title": title,
    }])
    .to_string();
    cm_with_data_dir(data_dir)
        .args(["deposit", "--exchanges", &exchanges])
        .assert()
        .success();
    let assert = cm_with_data_dir(data_dir)
        .args(["recall", "-j"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let json: Value = serde_json::from_str(&stdout).expect("recall -j must emit valid JSON");
    json["entries"][0]["id"]
        .as_str()
        .expect("first recall entry should expose an `id` field")
        .to_string()
}

// ---------------- Round-trip across every read/write subcommand ----------------

#[test]
fn round_trip_deposit_recall_get_update_forget_stats() {
    let dir = tempdir().unwrap();
    let id = deposit_one_and_extract_id(dir.path(), "round trip title", "round trip body");

    // `format_get_view` prints the full uuid on the `id:` line.
    cm_with_data_dir(dir.path())
        .args(["get", &id])
        .assert()
        .success()
        .stdout(contains(id.as_str()));

    // `format_update_ack` prints `updated: <id>` on success.
    cm_with_data_dir(dir.path())
        .args(["update", &id, "--title", "renamed"])
        .assert()
        .success()
        .stdout(contains("updated:"));

    // `format_forget_ack` prints `forgotten: <n>` on success.
    cm_with_data_dir(dir.path())
        .args(["forget", &id])
        .assert()
        .success()
        .stdout(contains("forgotten: 1"));

    // Stats survives the forget pass and still emits the counter block.
    cm_with_data_dir(dir.path())
        .args(["stats"])
        .assert()
        .success()
        .stdout(contains("active:"));
}

// ---------------- Store stub ----------------

#[test]
fn store_stub_points_users_to_curator_ui() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args(["store"])
        .assert()
        .success()
        .stdout(contains("Curator"))
        .stdout(contains("cm serve --web"));
}

#[test]
fn store_stub_rejects_removed_auto_scope_selector() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args(["store", "--scope", "auto"])
        .assert()
        .failure()
        .stderr(contains("instead of scope='auto'"));
}

#[test]
fn store_stub_accepts_current_scope_selectors() {
    for scope in ["cwd_inferred", "global/project:helioy"] {
        let dir = tempdir().unwrap();
        cm_with_data_dir(dir.path())
            .args(["store", "--scope", scope])
            .assert()
            .success()
            .stdout(contains("Curator"))
            .stdout(contains("cm serve --web"));
    }
}

// ---------------- Init ----------------

#[test]
fn init_writes_local_config_file_in_cwd() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("cm")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    assert!(
        dir.path().join(".cm.config.toml").exists(),
        "cm init should write .cm.config.toml into the current working directory"
    );
}

// ---------------- Browse ----------------

#[test]
fn browse_lists_seeded_entries_in_human_output() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[{"user":"q1","assistant":"a1","title":"alpha entry"},{"user":"q2","assistant":"a2","title":"beta entry"}]"#,
        ])
        .assert()
        .success();
    cm_with_data_dir(dir.path())
        .args(["browse"])
        .assert()
        .success()
        .stderr(contains("using scope='cwd_inferred'"))
        .stdout(contains("alpha entry"))
        .stdout(contains("beta entry"));
}

#[test]
fn browse_with_limit_caps_entry_count() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[
                {"user":"q1","assistant":"a1","title":"one"},
                {"user":"q2","assistant":"a2","title":"two"},
                {"user":"q3","assistant":"a3","title":"three"}
            ]"#,
        ])
        .assert()
        .success();
    let assert = cm_with_data_dir(dir.path())
        .args(["browse", "--limit", "2", "-j"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let json: Value = serde_json::from_str(&stdout).expect("browse -j must emit valid JSON");
    let entries = json["entries"]
        .as_array()
        .expect("browse view should expose an `entries` array");
    assert!(
        entries.len() <= 2,
        "with --limit 2, expected at most 2 entries (got {})",
        entries.len()
    );
}

#[test]
fn browse_scope_filters_exact_scope() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--scope",
            "global",
            "--exchanges",
            r#"[{"user":"q1","assistant":"a1","title":"global entry"}]"#,
        ])
        .assert()
        .success();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--scope",
            "global/project:helioy",
            "--exchanges",
            r#"[{"user":"q2","assistant":"a2","title":"project entry"}]"#,
        ])
        .assert()
        .success();

    let assert = cm_with_data_dir(dir.path())
        .args(["browse", "--scope", "global/project:helioy", "-j"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let json: Value = serde_json::from_str(&stdout).expect("browse -j must emit valid JSON");
    let titles: Vec<&str> = json["entries"]
        .as_array()
        .expect("browse view should expose entries")
        .iter()
        .map(|entry| entry["title"].as_str().expect("entry title is string"))
        .collect();
    assert_eq!(titles, vec!["project entry"]);
    assert!(json.get("advisory").is_none());
    assert!(json.get("resolution").is_none());
}

#[test]
fn browse_cwd_inferred_scope_can_emit_resolution_metadata() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[{"user":"q1","assistant":"a1","title":"auto scope entry"}]"#,
        ])
        .assert()
        .success();

    let assert = cm_with_data_dir(dir.path())
        .args([
            "browse",
            "--scope",
            "cwd_inferred",
            "--cwd",
            dir.path().to_str().expect("tempdir should be utf-8"),
            "--include-resolution",
            "-j",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let json: Value = serde_json::from_str(&stdout).expect("browse -j must emit valid JSON");
    assert_eq!(json["resolution"]["requested_scope"], "cwd_inferred");
    assert_eq!(json["resolution"]["resolved_scope"], "global");
    assert_eq!(json["resolution"]["scope_mode"], "resolved");
}

#[test]
fn browse_invalid_kind_uses_capability_error() {
    let dir = tempdir().unwrap();
    let expected = parse_kind("memo").unwrap_err();
    cm_with_data_dir(dir.path())
        .args(["browse", "--kind", "memo"])
        .assert()
        .failure()
        .stderr(contains(expected));
}

// ---------------- Export ----------------

#[test]
fn export_to_stdout_emits_valid_json_with_top_level_shape() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[{"user":"u","assistant":"a","title":"export sample"}]"#,
        ])
        .assert()
        .success();
    let assert = cm_with_data_dir(dir.path())
        .args(["export"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let json: Value =
        serde_json::from_str(&stdout).expect("cm export must emit serde_json-parsable JSON");
    assert!(
        json.get("entries").is_some(),
        "export top-level should expose `entries`"
    );
    assert!(
        json.get("scopes").is_some(),
        "export top-level should expose `scopes`"
    );
    assert!(
        json.get("exported_at").is_some(),
        "export top-level should expose `exported_at`"
    );
    assert_eq!(
        json["count"].as_u64().expect("count should be a number"),
        1,
        "count should reflect the single deposited entry"
    );
}

// ---------------- Update preserves unspecified fields ----------------

#[test]
fn update_title_preserves_body_content() {
    let dir = tempdir().unwrap();
    let id = deposit_one_and_extract_id(dir.path(), "original title", "preserved body marker");
    cm_with_data_dir(dir.path())
        .args(["update", &id, "--title", "renamed title"])
        .assert()
        .success();
    cm_with_data_dir(dir.path())
        .args(["get", &id])
        .assert()
        .success()
        .stdout(contains("renamed title"))
        .stdout(contains("preserved body marker"));
}

// ---------------- Forget multi-id ----------------

#[test]
fn forget_positional_accepts_multiple_ids() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[
                {"user":"q1","assistant":"a1","title":"first to forget"},
                {"user":"q2","assistant":"a2","title":"second to forget"}
            ]"#,
        ])
        .assert()
        .success();
    let assert = cm_with_data_dir(dir.path())
        .args(["recall", "-j"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let entries = json["entries"]
        .as_array()
        .expect("recall view should expose an `entries` array");
    assert_eq!(entries.len(), 2, "deposit should yield two visible entries");
    let id1 = entries[0]["id"].as_str().unwrap().to_string();
    let id2 = entries[1]["id"].as_str().unwrap().to_string();

    cm_with_data_dir(dir.path())
        .args(["forget", &id1, &id2])
        .assert()
        .success()
        .stdout(contains("forgotten: 2"));
}

// ---------------- Completions ----------------

#[test]
fn completions_bash_emits_completion_script() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("_cm()"));
}

// ---------------- Recall query path ----------------

#[test]
fn recall_query_finds_seeded_entry_by_keyword() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[{"user":"distinctkeyword body","assistant":"reply","title":"keyword test"}]"#,
        ])
        .assert()
        .success();
    cm_with_data_dir(dir.path())
        .args(["recall", "distinctkeyword"])
        .assert()
        .success()
        .stdout(contains("keyword test"));
}

#[test]
fn recall_without_scope_reports_default_advisory_on_stderr() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[{"user":"default scope body","assistant":"reply","title":"default scope test"}]"#,
        ])
        .assert()
        .success();
    cm_with_data_dir(dir.path())
        .args(["recall"])
        .assert()
        .success()
        .stdout(contains("default scope test"))
        .stderr(contains(RECALL_SCOPE_DEFAULT_ADVISORY));
}

// ---------------- Stats counters ----------------

#[test]
fn stats_reports_active_counter_after_deposit() {
    let dir = tempdir().unwrap();
    cm_with_data_dir(dir.path())
        .args([
            "deposit",
            "--exchanges",
            r#"[{"user":"u","assistant":"a","title":"stats counter test"}]"#,
        ])
        .assert()
        .success();
    cm_with_data_dir(dir.path())
        .args(["stats"])
        .assert()
        .success()
        .stdout(contains("active:"))
        .stdout(contains("scopes:"))
        .stdout(contains("relations:"));
}

#[test]
fn stats_invalid_tag_sort_uses_capability_error() {
    let dir = tempdir().unwrap();
    let expected = parse_tag_sort("recent").unwrap_err();
    cm_with_data_dir(dir.path())
        .args(["stats", "--tag-sort", "recent"])
        .assert()
        .failure()
        .stderr(contains(expected));
}
