//! SQLite persistence layer for context-matters.
//!
//! Implements the `ContextStore` trait from `cm-core` using sqlx
//! with a dual-pool architecture (1 writer, 4 readers).

pub mod config;
pub mod dedup;
pub mod project;
pub mod schema;
pub mod sqlite;

pub use config::{CONFIG_FILENAME, Config, config_template, load as load_config};
pub use dedup::{check_duplicate, recompute_hash_for_update};
pub use project::{default_base_dir, ensure_data_dir, resolve_home_dir};
pub use schema::{create_pools, run_migrations, wal_checkpoint};
pub use sqlite::CmStore;
