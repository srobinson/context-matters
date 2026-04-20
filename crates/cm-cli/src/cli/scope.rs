//! `--scope` resolution helpers shared by every read/write subcommand.
//!
//! Two flavors are exposed:
//!
//! * [`resolve_scope`] for *recall-style* commands where omission means
//!   "default to `global` and walk the ancestor chain". Returns a `String`.
//! * [`resolve_scope_filter`] for *browse-style* commands where omission
//!   means "no filter, return entries from every scope". Returns
//!   `Option<String>` so the capability layer can keep `scope_path = None`.
//!
//! Both helpers always print a one-line stderr advisory pointing users at
//! `cm stats` for scope discovery — never gated on TTY — so users who pipe
//! stdout to a file or another command still see the note.
//!
//! Tests assert on the substring `"no --scope specified"` (recall flavor)
//! and `"browsing all scopes"` (filter flavor); keep both stable.

use crate::cli::colors::Colors;

/// Recall-flavor advisory body. Tests grep for `"no --scope specified"`.
const ADVISORY_BODY_RECALL: &str =
    "no --scope specified, searching 'global'. run `cm stats` to list all scopes.";

/// Filter-flavor advisory body. Tests grep for `"browsing all scopes"`.
const ADVISORY_BODY_FILTER: &str =
    "no --scope specified, browsing all scopes. run `cm stats` to list all scopes.";

/// Build a colorized advisory line. Pure: takes a `Colors` set + body string
/// and returns the rendered line. Split out so unit tests can inspect the
/// rendering without capturing stderr, and so both [`resolve_scope`] and
/// [`resolve_scope_filter`] share a single rendering path.
fn advisory(c: &Colors, body: &str) -> String {
    format!("{}note:{} {}", c.dim, c.reset, body)
}

/// Resolve `--scope` for recall-style commands (defaults to `"global"`).
///
/// * `Some("foo")` → returns `"foo"`, no I/O.
/// * `Some("")` → treated as omitted (defaults to `"global"` with advisory).
/// * `None` → returns `"global"` and prints the advisory to stderr.
pub fn resolve_scope(explicit: Option<&str>) -> String {
    match explicit {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            eprintln!("{}", advisory(&Colors::stderr(), ADVISORY_BODY_RECALL));
            "global".to_string()
        }
    }
}

/// Resolve `--scope` for browse-style commands (omission == no filter).
///
/// * `Some("foo")` → returns `Some("foo")`, no I/O.
/// * `Some("")` → treated as omitted (returns `None` with advisory).
/// * `None` → returns `None` and prints the filter advisory to stderr.
pub fn resolve_scope_filter(explicit: Option<&str>) -> Option<String> {
    match explicit {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        _ => {
            eprintln!("{}", advisory(&Colors::stderr(), ADVISORY_BODY_FILTER));
            None
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
    fn filter_explicit_value_returns_some() {
        assert_eq!(
            resolve_scope_filter(Some("project:cm")),
            Some("project:cm".to_string())
        );
    }

    #[test]
    fn filter_none_returns_none() {
        assert_eq!(resolve_scope_filter(None), None);
    }

    #[test]
    fn filter_empty_string_treated_as_none() {
        assert_eq!(resolve_scope_filter(Some("")), None);
    }

    #[test]
    fn advisory_bodies_contain_required_substrings() {
        // Stable wording assertions. If either substring changes, the
        // tests in `tests/cli_integration.rs` (ALP-1784) that grep stderr
        // need to change in the same commit.
        assert!(ADVISORY_BODY_RECALL.contains("no --scope specified"));
        assert!(ADVISORY_BODY_RECALL.contains("cm stats"));
        assert!(ADVISORY_BODY_FILTER.contains("no --scope specified"));
        assert!(ADVISORY_BODY_FILTER.contains("browsing all scopes"));
        assert!(ADVISORY_BODY_FILTER.contains("cm stats"));
    }

    #[test]
    fn advisory_renders_with_disabled_colors_to_plain_text() {
        // When colors are disabled (NO_COLOR / non-tty / TERM=dumb),
        // the rendered advisory is plain ASCII with no escape bytes.
        let plain = advisory(&Colors::for_tty(false), ADVISORY_BODY_RECALL);
        assert_eq!(format!("note: {ADVISORY_BODY_RECALL}"), plain);
    }

    #[test]
    fn advisory_renders_with_enabled_colors_to_ansi_wrapped_text() {
        let prior_no_color = std::env::var_os("NO_COLOR");
        let prior_term = std::env::var_os("TERM");
        // SAFETY: the test snapshots and restores both variables before
        // asserting. No fallible code runs while the process env is patched.
        unsafe {
            std::env::remove_var("NO_COLOR");
            std::env::set_var("TERM", "xterm-256color");
        }
        // With colors enabled, the rendered string starts with the dim
        // escape and ends with the body. We assert on byte content to
        // catch any drift in the format string composition.
        let colored = advisory(&Colors::for_tty(true), ADVISORY_BODY_RECALL);
        // SAFETY: restores the exact environment captured above.
        unsafe {
            match prior_no_color {
                Some(v) => std::env::set_var("NO_COLOR", v),
                None => std::env::remove_var("NO_COLOR"),
            }
            match prior_term {
                Some(v) => std::env::set_var("TERM", v),
                None => std::env::remove_var("TERM"),
            }
        }
        assert!(colored.starts_with("\x1b[2m"), "expected dim prefix");
        assert!(colored.contains("\x1b[0m"), "expected reset escape");
        assert!(colored.contains(ADVISORY_BODY_RECALL));
    }
}
