//! CLI command handlers for context-matters.

#[path = "generated_help.rs"]
pub mod generated_help;

use anyhow::Result;
use cm_core::ContextStore;
use cm_store::CmStore;

/// Display store statistics on stdout.
pub async fn cmd_stats(store: &CmStore) -> Result<()> {
    let stats = store.stats().map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("context-matters v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Active entries:     {}", stats.active_entries);
    println!("Superseded entries: {}", stats.superseded_entries);
    println!("Scopes:             {}", stats.scopes);
    println!("Relations:          {}", stats.relations);
    println!("Database size:      {} bytes", stats.db_size_bytes);

    if !stats.entries_by_kind.is_empty() {
        println!();
        println!("By kind:");
        let mut kinds: Vec<_> = stats.entries_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in kinds {
            println!("  {kind:15} {count}");
        }
    }

    if !stats.entries_by_scope.is_empty() {
        println!();
        println!("By scope:");
        let mut scopes: Vec<_> = stats.entries_by_scope.iter().collect();
        scopes.sort_by_key(|(path, _)| (*path).clone());
        for (scope, count) in scopes {
            println!("  {scope:40} {count}");
        }
    }

    Ok(())
}
