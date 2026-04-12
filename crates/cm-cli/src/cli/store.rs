//! `cm store` — discoverable stub that points users at the Curator web UI.
//!
//! Locked decision (ALP-1781): a full CLI handler for entry creation carries
//! significant UX overhead (multi-line body via `$EDITOR`, flag-heavy
//! metadata, confidence enum validation) for a workflow that has no real CLI
//! customer. The clap surface in [`super::cli_def`] is registered with the
//! generated `STORE_*` help constants so `cm store --help` and
//! `cm --markdown-help` document every flag, but invocation prints a short
//! pointer to the Curator UI and exits 0.
//!
//! All flags parse and are silently dropped. If a real handler is ever
//! needed, this file is the hook point.
//!
//! Agents continue to use the MCP `cx_store` tool, which lives in
//! `crates/cm-cli/src/mcp/tools/store.rs` and is unaffected by this stub.

use anyhow::Result;

use crate::cli::colors::Colors;

/// `cm store` handler. Synchronous because it touches no I/O beyond
/// `println!`. Returns `Ok(())` after printing; the binary exits 0.
pub fn run() -> Result<()> {
    let c = Colors::stdout();
    println!(
        "{bold}cm store{reset} is not exposed as a CLI handler.",
        bold = c.bold,
        reset = c.reset,
    );
    println!();
    println!(
        "Direct entry creation lives in the {bold}Curator{reset} web UI:",
        bold = c.bold,
        reset = c.reset,
    );
    println!();
    println!(
        "    {cyan}cm serve --web{reset}",
        cyan = c.cyan,
        reset = c.reset
    );
    println!();
    println!(
        "Then open {cyan}http://localhost:7878/curator{reset} in your browser.",
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

    /// `run` must always return `Ok(())`. The stub is by definition
    /// infallible — if this regresses, callers in `main.rs` would start
    /// propagating errors that should never have existed.
    #[test]
    fn run_returns_ok() {
        assert!(run().is_ok());
    }
}
