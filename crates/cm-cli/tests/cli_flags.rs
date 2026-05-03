//! Pure flag-surface tests for the `cm` CLI.
//!
//! Spawns the compiled binary via `assert_cmd` and exercises every
//! subcommand's `--help` output, the hidden `--markdown-help` and
//! `--generate-man-pages` flags, the `NO_COLOR` / `TERM=dumb` env opt-outs,
//! and the unknown-subcommand exit code. No test in this file touches a
//! database; all assertions live entirely in the clap layer and run in
//! milliseconds.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::tempdir;

/// Spawn a fresh handle to the compiled `cm` binary. Mirrors the helper
/// in `tests/config_startup_test.rs` so cm-cli integration tests share
/// one entry point.
fn cm() -> Command {
    Command::cargo_bin("cm").unwrap()
}

// ---------------- Root help surface ----------------

#[test]
fn root_long_help_lists_all_fourteen_subcommands() {
    cm().arg("--help")
        .assert()
        .success()
        .stdout(contains("recall"))
        .stdout(contains("search"))
        .stdout(contains("browse"))
        .stdout(contains("get"))
        .stdout(contains("stats"))
        .stdout(contains("store"))
        .stdout(contains("update"))
        .stdout(contains("deposit"))
        .stdout(contains("forget"))
        .stdout(contains("init"))
        .stdout(contains("serve"))
        .stdout(contains("web"))
        .stdout(contains("export"))
        .stdout(contains("completions"));
}

#[test]
fn root_short_help_lists_all_fourteen_subcommands() {
    cm().arg("-h")
        .assert()
        .success()
        .stdout(contains("recall"))
        .stdout(contains("search"))
        .stdout(contains("browse"))
        .stdout(contains("get"))
        .stdout(contains("stats"))
        .stdout(contains("store"))
        .stdout(contains("update"))
        .stdout(contains("deposit"))
        .stdout(contains("forget"))
        .stdout(contains("init"))
        .stdout(contains("serve"))
        .stdout(contains("web"))
        .stdout(contains("export"))
        .stdout(contains("completions"));
}

#[test]
fn root_long_help_uses_read_write_admin_groups() {
    cm().arg("--help")
        .assert()
        .success()
        .stdout(contains("READ Commands"))
        .stdout(contains("WRITE Commands"))
        .stdout(contains("ADMIN Commands"))
        .stdout(contains("Examples"))
        .stdout(contains("Scope Resolution"))
        .stdout(contains("search requires --scope"))
        .stdout(contains("browse starts at cwd_inferred"));
}

#[test]
fn root_long_help_bridges_cli_and_mcp_names() {
    cm().arg("--help")
        .assert()
        .success()
        .stdout(contains(
            "This CLI mirrors the MCP tool surface. From a shell, use cm <command>.",
        ))
        .stdout(contains(
            "From an MCP client, the same operations are exposed as cx_<command>.",
        ))
        .stdout(contains("Run cm serve to start the MCP server on stdio."));
}

#[test]
fn root_long_help_surfaces_read_scope_contracts() {
    cm().arg("--help")
        .assert()
        .success()
        .stdout(contains(
            "recall      Search one scope plus ancestors. Default: global.",
        ))
        .stdout(contains(
            "search      Content search across scopes. Requires --scope.",
        ))
        .stdout(contains(
            "browse      Filtered inventory with pagination. Default: cwd_inferred.",
        ));
}

#[test]
fn root_long_help_promotes_startup_and_write_examples() {
    cm().arg("--help")
        .assert()
        .success()
        .stdout(contains("cm serve"))
        .stdout(contains("start MCP server on stdio"))
        .stdout(contains("cm init"))
        .stdout(contains("write config to ./.cm.config.toml"))
        .stdout(contains("cm init --global"))
        .stdout(contains(
            "write config to ~/.context-matters/.cm.config.toml",
        ))
        .stdout(contains("cm web --open"))
        .stdout(contains("open http://localhost:3141/"))
        .stdout(contains("cm forget 019d09ed-7a4f-7693"))
        .stdout(contains("mark entry forgotten by id"));
}

#[test]
fn root_long_help_avoids_obsolete_web_guidance() {
    cm().arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Curator").not())
        .stdout(predicates::str::contains("cm serve --web").not())
        .stdout(predicates::str::contains("tiered FTS5").not())
        .stdout(contains("Create a new entry via the web UI"))
        .stdout(contains("Mark entries forgotten"));
}

#[test]
fn root_short_help_surfaces_read_scope_contracts() {
    cm().arg("-h")
        .assert()
        .success()
        .stdout(contains(
            "recall      Search one scope plus ancestors. Default: global.",
        ))
        .stdout(contains(
            "search      Content search across scopes. Requires --scope.",
        ))
        .stdout(contains(
            "browse      Filtered inventory with pagination. Default: cwd_inferred.",
        ));
}

#[test]
fn no_args_prints_long_help_and_exits_zero() {
    cm().assert()
        .success()
        .stdout(contains("READ Commands"))
        .stdout(contains("WRITE Commands"))
        .stdout(contains("ADMIN Commands"));
}

#[test]
fn version_prints_crate_version() {
    cm().arg("--version")
        .assert()
        .success()
        .stdout(contains(format!("cm {}", env!("CARGO_PKG_VERSION"))));
}

// ---------------- Per-subcommand help surface ----------------

#[test]
fn recall_help_shows_per_arg_descriptions() {
    cm().args(["recall", "--help"])
        .assert()
        .success()
        .stdout(contains("FTS5 search query"))
        .stdout(contains("Scope selector"))
        .stdout(contains("Filter by entry kind"));
}

#[test]
fn search_help_shows_per_arg_descriptions() {
    cm().args(["search", "--help"])
        .assert()
        .success()
        .stdout(contains("Required FTS5 search query"))
        .stdout(contains("exact path"))
        .stdout(contains("reserved value cwd_inferred"))
        .stdout(contains("structured subtree/set/all JSON"))
        .stdout(contains("Filter by entry kind"))
        .stdout(contains("Filter by tag"))
        .stdout(contains("Maximum number of results"))
        .stdout(contains("Pagination cursor"))
        .stdout(contains("Emit JSON instead of human-readable text"));
}

#[test]
fn browse_help_shows_per_arg_descriptions() {
    cm().args(["browse", "--help"])
        .assert()
        .success()
        .stdout(contains(
            "cm browse --scope cwd_inferred --cwd /path/to/repo",
        ))
        .stdout(contains("Preferred scope"))
        .stdout(contains("cwd_inferred"))
        .stdout(predicates::str::contains("--scope-path").not())
        .stdout(predicates::str::contains("--scope-mode").not())
        .stdout(contains(
            "Working directory used for cwd_inferred scope resolution",
        ))
        .stdout(contains("Include scope resolution metadata"))
        .stdout(contains("Filter by kind"))
        .stdout(contains("Pagination cursor"));
}

#[test]
fn get_help_shows_per_arg_descriptions() {
    cm().args(["get", "--help"])
        .assert()
        .success()
        .stdout(contains("Entry IDs to retrieve"))
        .stdout(contains("UUIDv7"));
}

#[test]
fn stats_help_shows_per_arg_descriptions() {
    cm().args(["stats", "--help"])
        .assert()
        .success()
        .stdout(contains("Tag sort order"));
}

#[test]
fn store_help_shows_per_arg_descriptions() {
    cm().args(["store", "--help"])
        .assert()
        .success()
        .stdout(contains("Entry title"))
        .stdout(contains("--scope"))
        .stdout(contains("cwd_inferred"))
        .stdout(predicates::str::contains("--scope-path").not())
        .stdout(contains("Confidence level"))
        .stdout(contains("Numeric priority"))
        .stdout(contains("cm web --open"))
        .stdout(contains("http://localhost:3141/"))
        .stdout(predicates::str::contains("Curator").not())
        .stdout(predicates::str::contains("cm serve --web").not());
}

#[test]
fn update_help_shows_per_arg_descriptions() {
    cm().args(["update", "--help"])
        .assert()
        .success()
        .stdout(contains("Entry ID to update"))
        .stdout(contains("Replace metadata"));
}

#[test]
fn deposit_help_shows_per_arg_descriptions() {
    cm().args(["deposit", "--help"])
        .assert()
        .success()
        .stdout(contains("JSON array of"))
        .stdout(contains("--scope"))
        .stdout(contains("cwd_inferred"))
        .stdout(predicates::str::contains("--scope-path").not())
        .stdout(contains("Optional conversation summary"));
}

#[test]
fn forget_help_shows_per_arg_descriptions() {
    cm().args(["forget", "--help"])
        .assert()
        .success()
        .stdout(contains("Entry IDs to forget"));
}

#[test]
fn init_help_shows_per_arg_descriptions() {
    cm().args(["init", "--help"])
        .assert()
        .success()
        .stdout(contains("Write to ~/.context-matters/"))
        .stdout(contains("write to ~/.context-matters/.cm.config.toml"))
        .stdout(contains("Overwrite an existing config file"));
}

#[test]
fn serve_help_shows_examples_block() {
    cm().args(["serve", "--help"])
        .assert()
        .success()
        .stdout(contains("MCP server on stdio"));
}

#[test]
fn web_help_shows_per_arg_descriptions() {
    cm().args(["web", "--help"])
        .assert()
        .success()
        .stdout(contains("Start the embedded web UI"))
        .stdout(contains("Open http://localhost:3141/ after starting"))
        .stdout(contains("Port to listen on. Defaults to 3141."))
        .stdout(contains("cm web --port 4000"));
}

#[test]
fn export_help_shows_per_arg_descriptions() {
    cm().args(["export", "--help"])
        .assert()
        .success()
        .stdout(contains("Filter export to scope selector"))
        .stdout(contains("cwd_inferred"))
        .stdout(predicates::str::contains("--scope-path").not())
        .stdout(contains("Export format"));
}

#[test]
fn migrated_scope_help_names_cwd_inferred_as_reserved_value() {
    for command in ["recall", "search", "store", "deposit", "browse", "export"] {
        cm().args([command, "--help"])
            .assert()
            .success()
            .stdout(contains("reserved value cwd_inferred"));
    }
}

#[test]
fn removed_scope_path_flag_is_rejected() {
    cm().args(["browse", "--scope-path", "global"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("unexpected argument '--scope-path'"));

    cm().args([
        "deposit",
        "--exchanges",
        r#"[{"user":"u","assistant":"a"}]"#,
        "--scope-path",
        "global",
    ])
    .assert()
    .failure()
    .code(2)
    .stderr(contains("unexpected argument '--scope-path'"));

    cm().args(["export", "--scope-path", "global"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("unexpected argument '--scope-path'"));
}

#[test]
fn completions_help_shows_per_arg_descriptions() {
    cm().args(["completions", "--help"])
        .assert()
        .success()
        .stdout(contains(
            "Target shell: bash, zsh, fish, powershell, elvish",
        ));
}

// ---------------- Hidden documentation flags ----------------

#[test]
fn markdown_help_emits_clap_markdown_to_stdout() {
    cm().arg("--markdown-help")
        .assert()
        .success()
        .stdout(contains("# Command-Line Help for"));
}

#[test]
fn generate_man_pages_writes_files_into_target_dir() {
    let dir = tempdir().unwrap();
    cm().arg("--generate-man-pages")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(contains("wrote man pages to"));
    let man_pages: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "1"))
        .collect();
    assert!(
        !man_pages.is_empty(),
        "expected at least one .1 man page in {}",
        dir.path().display()
    );
}

// ---------------- NO_COLOR / TERM=dumb opt-outs ----------------
//
// `cm completions bash` is the cleanest stdout target for these env-var
// tests: it routes through `clap_complete` (no compile-time color
// baking), needs no database, and emits a known-shape script. The
// `colors.rs` unit tests cover the runtime decision function directly;
// these integration tests prove the env vars do not break the binary
// and that a known-clean output path stays ANSI-free under both
// opt-outs.

#[test]
fn no_color_env_yields_ansi_free_completions_output() {
    let assert = cm()
        .env("NO_COLOR", "1")
        .args(["completions", "bash"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        !stdout.contains('\x1b'),
        "NO_COLOR=1 cm completions bash should not contain ANSI escapes"
    );
}

#[test]
fn term_dumb_yields_ansi_free_completions_output() {
    let assert = cm()
        .env("TERM", "dumb")
        .args(["completions", "bash"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        !stdout.contains('\x1b'),
        "TERM=dumb cm completions bash should not contain ANSI escapes"
    );
}

// ---------------- Exit codes ----------------

#[test]
fn unknown_subcommand_exits_with_code_two() {
    cm().arg("xyz").assert().failure().code(2);
}

#[test]
fn bad_completions_shell_exits_with_code_two() {
    cm().args(["completions", "tcsh"])
        .assert()
        .failure()
        .code(2);
}
