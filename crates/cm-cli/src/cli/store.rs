//! `cm store` — discoverable stub that points users at cm-web.
//!
//! Locked decision (ALP-1781): a full CLI handler for entry creation carries
//! significant UX overhead (multi-line body via `$EDITOR`, flag-heavy
//! metadata, confidence enum validation) for a workflow that has no real CLI
//! customer. The clap surface in [`super::cli_def`] is registered with the
//! generated `STORE_*` help constants so `cm store --help` and
//! `cm --markdown-help` document every flag. Valid invocations print a short
//! pointer to cm-web and exit 0.
//!
//! Accepted flags parse and are dropped after scope selector validation. If a
//! real handler is ever needed, this file is the hook point.
//!
//! Agents continue to use the MCP `cx_store` tool, which lives in
//! `crates/cm-cli/src/mcp/tools/store.rs` and is unaffected by this stub.

use anyhow::Result;
use cm_capabilities::scope::ScopeSelector;

use crate::cli::colors::Colors;
use crate::cli::errors::capability_error;
use crate::shared::normalize_scope_selector_input;

/// `cm store` handler. Synchronous because it touches no I/O beyond
/// `println!`. Validates the optional scope selector before printing so
/// removed public inputs fail the same way as MCP `cx_store`.
pub fn run(scope: Option<String>) -> Result<()> {
    if let Some(scope) = scope {
        let scope = normalize_scope_selector_input(&scope);
        ScopeSelector::parse(&scope).map_err(capability_error)?;
    }

    let c = Colors::stdout();
    println!(
        "{bold}cm store{reset} is not exposed as a CLI handler.",
        bold = c.bold,
        reset = c.reset,
    );
    println!();
    println!(
        "Direct entry creation lives in {bold}cm-web{reset}:",
        bold = c.bold,
        reset = c.reset,
    );
    println!();
    println!(
        "    {cyan}cm-web --open{reset}",
        cyan = c.cyan,
        reset = c.reset
    );
    println!();
    println!(
        "Or open {cyan}http://localhost:3141/{reset} in your browser.",
        cyan = c.cyan,
        reset = c.reset,
    );
    println!();
    println!(
        "Agents can also call the MCP tool {dim}cx_store{reset}.",
        dim = c.dim,
        reset = c.reset,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without a scope selector, `run` prints the stub message and exits
    /// successfully.
    #[test]
    fn run_returns_ok() {
        assert!(run(None).is_ok());
    }
}
