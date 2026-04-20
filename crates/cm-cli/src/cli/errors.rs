//! Top-level error rendering for the `cm` CLI.
//!
//! Renders an [`anyhow::Error`] chain to stderr with colored prefixes and
//! contextual hint blocks. Hints are matched on lower-cased substrings of
//! the formatted error message — additive, not exhaustive. As new error
//! shapes appear, add a new branch to [`hints_for`] and a matching test.

use crate::cli::colors::Colors;

/// Print an error chain to stderr with colored prefixes and contextual hints.
pub fn print_error(err: &anyhow::Error) {
    let c = Colors::stderr();
    eprintln!("{}error:{} {err}", c.red_bold, c.reset);
    for cause in err.chain().skip(1) {
        eprintln!("{}caused by:{} {cause}", c.yellow, c.reset);
    }

    let msg = format!("{err:#}").to_lowercase();
    for hint in hints_for(&msg) {
        eprintln!();
        eprintln!("{}hint:{} {hint}", c.cyan, c.reset);
    }
}

/// Pure hint matcher: pick the contextual hint strings that apply to the
/// lower-cased error message. Split out from [`print_error`] so the matching
/// logic is unit-testable without capturing stderr.
fn hints_for(msg_lower: &str) -> Vec<&'static str> {
    let mut hints = Vec::new();

    if msg_lower.contains("scope")
        && (msg_lower.contains("not found") || msg_lower.contains("invalid"))
    {
        hints.push("run `cm stats` to list all scopes in the store.");
    }

    if msg_lower.contains("uuid")
        || (msg_lower.contains("id")
            && (msg_lower.contains("invalid") || msg_lower.contains("not found")))
    {
        hints.push("use `cm browse` or `cm recall` to discover entry ids.");
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_not_found_emits_stats_hint() {
        let err = anyhow::anyhow!("scope not found: foo/bar");
        let lower = format!("{err:#}").to_lowercase();
        let hints = hints_for(&lower);
        assert!(
            hints
                .iter()
                .any(|h| h.contains("cm stats") && h.contains("scopes")),
            "expected scope hint, got {hints:?}"
        );
    }

    #[test]
    fn invalid_uuid_emits_browse_hint() {
        let err = anyhow::anyhow!("invalid uuid: not-a-uuid");
        let lower = format!("{err:#}").to_lowercase();
        let hints = hints_for(&lower);
        assert!(
            hints.iter().any(|h| h.contains("cm browse")),
            "expected id hint, got {hints:?}"
        );
    }

    #[test]
    fn unrecognized_message_emits_no_hints() {
        let err = anyhow::anyhow!("permission denied opening file");
        let lower = format!("{err:#}").to_lowercase();
        assert!(hints_for(&lower).is_empty());
    }

    #[test]
    fn print_error_does_not_panic_on_chain() {
        // Smoke test against the rendering pathway: a multi-cause anyhow
        // chain should round-trip through print_error without panicking.
        // We cannot easily capture stderr here, but the hint logic is
        // covered above; this test just guarantees the eprintln!s execute.
        let inner = anyhow::anyhow!("invalid uuid in row");
        let outer = inner.context("loading entry from store");
        print_error(&outer);
    }
}
