//! `--scope` resolution helper shared by every read/write subcommand.
//!
//! When the user omits `--scope`, the helper defaults to `"global"` and
//! prints a one-line stderr advisory pointing them at `cm stats` for scope
//! discovery. The advisory is always-on (not gated on TTY) so users who pipe
//! stdout to a file or another command still see the note.

use crate::cli::colors::Colors;

/// Stable advisory body. Tests assert on the substring `"no --scope specified"`
/// — keep this string and the substring in sync if either ever changes.
const ADVISORY_BODY: &str =
    "no --scope specified, searching 'global'. run `cm stats` to list all scopes.";

/// Build the colorized advisory line. Pure: takes a `Colors` set and returns
/// the rendered string. Split out from [`resolve_scope`] so unit tests can
/// inspect the body without capturing stderr.
fn advisory(c: &Colors) -> String {
    format!("{}note:{} {}", c.dim, c.reset, ADVISORY_BODY)
}

/// Resolve the `--scope` argument with a stderr advisory on default.
///
/// * `Some("foo")` → returns `"foo"`, no I/O.
/// * `Some("")` → treated as omitted (defaults to `"global"` with advisory).
/// * `None` → returns `"global"` and prints the advisory to stderr.
pub fn resolve_scope(explicit: Option<&str>) -> String {
    match explicit {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            eprintln!("{}", advisory(&Colors::stderr()));
            "global".to_string()
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
            "global/project:helioy"
        );
        assert_eq!(resolve_scope(Some("foo")), "foo");
    }

    #[test]
    fn none_defaults_to_global() {
        // The eprintln! lands on the test runner's captured stderr; we can't
        // assert on it portably, but we can confirm the function returns the
        // documented default and the path runs without panicking.
        assert_eq!(resolve_scope(None), "global");
    }

    #[test]
    fn empty_string_treated_as_none() {
        assert_eq!(resolve_scope(Some("")), "global");
    }

    #[test]
    fn advisory_body_contains_required_substrings() {
        // Stable wording assertions. If either substring changes, the
        // tests in `tests/cli_integration.rs` (ALP-1784) that grep stderr
        // need to change in the same commit.
        assert!(ADVISORY_BODY.contains("no --scope specified"));
        assert!(ADVISORY_BODY.contains("cm stats"));
    }

    #[test]
    fn advisory_renders_with_disabled_colors_to_plain_text() {
        // When colors are disabled (NO_COLOR / non-tty / TERM=dumb),
        // the rendered advisory is plain ASCII with no escape bytes.
        let plain = advisory(&Colors::for_tty(false));
        assert_eq!(format!("note: {ADVISORY_BODY}"), plain);
    }

    #[test]
    fn advisory_renders_with_enabled_colors_to_ansi_wrapped_text() {
        // With colors enabled, the rendered string starts with the dim
        // escape and ends with the body. We assert on byte content to
        // catch any drift in the format string composition.
        let colored = advisory(&Colors::for_tty(true));
        assert!(colored.starts_with("\x1b[2m"), "expected dim prefix");
        assert!(colored.contains("\x1b[0m"), "expected reset escape");
        assert!(colored.contains(ADVISORY_BODY));
    }
}
