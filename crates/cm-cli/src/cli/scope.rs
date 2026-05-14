//! `--scope` resolution helpers shared by every read/write subcommand.
//!
//! The helper exposed here is for deposit-style commands that still default to
//! `global`. Browse and recall defaults live in `cm-capabilities` and are
//! returned as capability advisories.
//!
//! Scope advisories always print a one-line stderr message pointing users at
//! `cm stats` for scope discovery, never gated on TTY, so users who pipe
//! stdout to a file or another command still see the note.
//!
//! Tests assert on the substring `"no --scope specified"`; keep it stable.

use crate::cli::colors::Colors;
use crate::shared::normalize_scope_selector_input;
use cm_capabilities::recall::RECALL_SCOPE_DEFAULT_ADVISORY;

/// Build a colorized advisory line. Pure: takes a `Colors` set + body string
/// and returns the rendered line. Split out so unit tests can inspect the
/// rendering without capturing stderr.
fn advisory(c: &Colors, body: &str) -> String {
    format!("{}note:{} {}", c.dim, c.reset, body)
}

pub fn print_advisory(body: &str) {
    eprintln!("{}", advisory(&Colors::stderr(), body));
}

/// Resolve `--scope` for legacy CLI commands that still default locally.
///
/// * `Some("foo")` → returns a structured path selector, no I/O.
/// * `Some("")` → treated as omitted with a structured global selector.
/// * `None` → returns a structured global selector and prints the advisory to stderr.
pub fn resolve_scope(explicit: Option<&str>) -> String {
    match explicit {
        Some(s) if !s.is_empty() => normalize_scope_selector_input(s),
        _ => {
            print_advisory(RECALL_SCOPE_DEFAULT_ADVISORY);
            normalize_scope_selector_input("global")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_value_returns_unchanged() {
        assert_eq!(
            resolve_scope(Some("global/project:helioy")),
            r#"{"kind":"path","path":"global/project:helioy"}"#
        );
        assert_eq!(
            resolve_scope(Some("foo")),
            r#"{"kind":"path","path":"foo"}"#
        );
    }

    #[test]
    fn none_defaults_to_global() {
        // The eprintln! lands on the test runner's captured stderr; we can't
        // assert on it portably, but we can confirm the function returns the
        // documented default and the path runs without panicking.
        assert_eq!(resolve_scope(None), r#"{"kind":"path","path":"global"}"#);
    }

    #[test]
    fn empty_string_treated_as_none() {
        assert_eq!(
            resolve_scope(Some("")),
            r#"{"kind":"path","path":"global"}"#
        );
    }

    #[test]
    fn advisory_bodies_contain_required_substrings() {
        // Stable wording assertions. If either substring changes, the
        // tests in `tests/cli_integration.rs` (ALP-1784) that grep stderr
        // need to change in the same commit.
        assert!(RECALL_SCOPE_DEFAULT_ADVISORY.contains("no --scope specified"));
        assert!(RECALL_SCOPE_DEFAULT_ADVISORY.contains("cm stats"));
    }

    #[test]
    fn advisory_renders_with_disabled_colors_to_plain_text() {
        // When colors are disabled (NO_COLOR / non-tty / TERM=dumb),
        // the rendered advisory is plain ASCII with no escape bytes.
        let plain = advisory(&Colors::for_tty(false), RECALL_SCOPE_DEFAULT_ADVISORY);
        assert_eq!(format!("note: {RECALL_SCOPE_DEFAULT_ADVISORY}"), plain);
    }

    #[test]
    fn advisory_renders_with_enabled_colors_to_ansi_wrapped_text() {
        // With colors enabled, the rendered string starts with the dim
        // escape and ends with the body. We assert on byte content to
        // catch any drift in the format string composition.
        let colored = advisory(&Colors::enabled(), RECALL_SCOPE_DEFAULT_ADVISORY);
        assert!(colored.starts_with("\x1b[2m"), "expected dim prefix");
        assert!(colored.contains("\x1b[0m"), "expected reset escape");
        assert!(colored.contains(RECALL_SCOPE_DEFAULT_ADVISORY));
    }
}
