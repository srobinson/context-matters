//! Terminal color support that respects `NO_COLOR`, `TERM=dumb`, and TTY detection.
//!
//! Returns ANSI escape codes when color output is appropriate, empty strings
//! otherwise. Hand-rolled to keep zero runtime color dependencies; `color_print`
//! is admitted as a workspace dep only for compile-time `cstr!` use inside clap
//! `after_help` blocks.

use std::io::IsTerminal;

/// Resolved ANSI escape sequences for the eight colors used by the CLI.
///
/// Constructed via [`Colors::stdout`] or [`Colors::stderr`]; both checks honor
/// the `NO_COLOR` convention (<https://no-color.org/>), `TERM=dumb`, and
/// whether the corresponding stream is a terminal.
pub struct Colors {
    pub bold: &'static str,
    pub dim: &'static str,
    pub reset: &'static str,
    pub cyan: &'static str,
    pub yellow: &'static str,
    pub red: &'static str,
    pub green: &'static str,
    pub red_bold: &'static str,
}

impl Colors {
    /// Color set appropriate for `stdout`.
    pub fn stdout() -> Self {
        Self::for_tty(std::io::stdout().is_terminal())
    }

    /// Color set appropriate for `stderr`.
    pub fn stderr() -> Self {
        Self::for_tty(std::io::stderr().is_terminal())
    }

    /// Decide enabled vs disabled given a pre-resolved TTY flag.
    ///
    /// Split out from [`Colors::stdout`]/[`Colors::stderr`] so unit tests can
    /// exercise the `NO_COLOR`/`TERM` branches without depending on the real
    /// stdio handles. `IsTerminal` is a sealed trait, so a fake stream type
    /// is not implementable; passing a `bool` sidesteps the seal.
    pub(crate) fn for_tty(is_terminal: bool) -> Self {
        if should_colorize(is_terminal) {
            Self::enabled()
        } else {
            Self::disabled()
        }
    }

    const fn enabled() -> Self {
        Self {
            bold: "\x1b[1m",
            dim: "\x1b[2m",
            reset: "\x1b[0m",
            cyan: "\x1b[36m",
            yellow: "\x1b[33m",
            red: "\x1b[31m",
            green: "\x1b[32m",
            red_bold: "\x1b[1;31m",
        }
    }

    const fn disabled() -> Self {
        Self {
            bold: "",
            dim: "",
            reset: "",
            cyan: "",
            yellow: "",
            red: "",
            green: "",
            red_bold: "",
        }
    }
}

/// Decide whether a stream that is `is_terminal` should receive ANSI escapes.
///
/// Order of checks matches the `NO_COLOR` spec: explicit opt-out wins, then
/// dumb-terminal opt-out, then the TTY flag.
fn should_colorize(is_terminal: bool) -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var("TERM").is_ok_and(|t| t == "dumb") {
        return false;
    }
    is_terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_returns_empty_strings_for_every_field() {
        let c = Colors::disabled();
        assert!(c.bold.is_empty());
        assert!(c.dim.is_empty());
        assert!(c.reset.is_empty());
        assert!(c.cyan.is_empty());
        assert!(c.yellow.is_empty());
        assert!(c.red.is_empty());
        assert!(c.green.is_empty());
        assert!(c.red_bold.is_empty());
    }

    #[test]
    fn enabled_returns_ansi_escapes_for_every_field() {
        let c = Colors::enabled();
        assert_eq!(c.bold, "\x1b[1m");
        assert_eq!(c.dim, "\x1b[2m");
        assert_eq!(c.reset, "\x1b[0m");
        assert_eq!(c.cyan, "\x1b[36m");
        assert_eq!(c.yellow, "\x1b[33m");
        assert_eq!(c.red, "\x1b[31m");
        assert_eq!(c.green, "\x1b[32m");
        assert_eq!(c.red_bold, "\x1b[1;31m");
    }

    /// `Colors::stdout`/`stderr` must not panic regardless of the test
    /// harness's terminal state — they should always return a valid struct.
    #[test]
    fn stdout_and_stderr_constructors_do_not_panic() {
        let _ = Colors::stdout();
        let _ = Colors::stderr();
    }

    #[test]
    fn no_color_env_forces_disabled_even_on_tty() {
        // Snapshot the prior value so concurrent test threads can survive
        // each other if they happen to alternate around this case. Cargo
        // runs unit tests in parallel; mutating process env is racy by
        // construction. The remove-or-restore in the cleanup arm minimizes
        // the window where another thread observes the wrong value.
        let prior = std::env::var_os("NO_COLOR");
        // SAFETY: this test is the only NO_COLOR mutator in the module; the
        // cleanup arm restores the prior value before returning so any later
        // test sees the original environment.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        let result = should_colorize(true);
        unsafe {
            match prior {
                Some(v) => std::env::set_var("NO_COLOR", v),
                None => std::env::remove_var("NO_COLOR"),
            }
        }
        assert!(!result, "NO_COLOR=1 must force colorization off");
    }
}
