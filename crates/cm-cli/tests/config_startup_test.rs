//! Startup integration tests for config validation.
//!
//! Verify the binary exits with a clear error message when config is invalid.

use assert_cmd::Command;

fn cm() -> Command {
    Command::cargo_bin("cm").unwrap()
}

#[test]
fn serve_exits_with_error_on_relative_data_dir() {
    cm().arg("serve")
        .env("CM_DATA_DIR", "relative/path")
        .assert()
        .failure()
        .stderr(predicates::str::contains("must be an absolute path"));
}

#[test]
fn serve_exits_with_error_on_empty_data_dir() {
    cm().arg("serve")
        .env("CM_DATA_DIR", "")
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be empty"));
}

#[test]
fn stats_exits_with_error_on_relative_data_dir() {
    cm().arg("stats")
        .env("CM_DATA_DIR", "relative/path")
        .assert()
        .failure()
        .stderr(predicates::str::contains("must be an absolute path"));
}
